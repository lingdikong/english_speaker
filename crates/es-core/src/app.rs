//! `AppCore`：核心层对外的唯一门面。FFI（移动端）和 JSON 桥（桌面端）都只调用它。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cache::{DEFAULT_LIMIT_BYTES, LIMIT_SETTING};
use crate::error::{CoreError, Result};
use crate::events::{CoreEvent, EventListener};
use crate::model::*;
use crate::player::{Player, PlayerCmd};
use crate::ports::{AudioCodec, AudioSink, Storage, TtsEngine};
use crate::synth::Synthesizer;
use crate::text;

pub struct AppConfig {
    /// 应用私有数据目录（数据库、音频缓存）。
    pub data_dir: PathBuf,
    /// 默认音色；为空时取引擎第一个音色。
    pub default_voice: Option<String>,
    /// 导入后立即在后台开始合成。
    pub auto_synthesize: bool,
    /// 缓存池默认上限；用户通过 `set_cache_limit` 修改后以数据库中的设置为准。
    pub cache_limit_bytes: u64,
}

impl AppConfig {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            default_voice: None,
            auto_synthesize: true,
            cache_limit_bytes: DEFAULT_LIMIT_BYTES,
        }
    }
}

pub struct AppDeps {
    pub storage: Arc<dyn Storage>,
    pub tts: Arc<dyn TtsEngine>,
    pub codec: Arc<dyn AudioCodec>,
    pub sink: Arc<dyn AudioSink>,
    pub listener: Arc<dyn EventListener>,
}

/// 合成线程与播放线程共享的依赖与运行时状态。
pub(crate) struct Shared {
    pub storage: Arc<dyn Storage>,
    pub tts: Arc<dyn TtsEngine>,
    pub codec: Arc<dyn AudioCodec>,
    pub listener: Arc<dyn EventListener>,
    pub audio_dir: PathBuf,
    pub default_voice: String,
    pub cache_limit: AtomicU64,
    /// 正在播放 / 正在合成的文章不会被缓存池淘汰。
    pub now_playing: Mutex<Option<ArticleId>>,
    pub synthesizing: Mutex<Option<ArticleId>>,
}

impl Shared {
    pub fn emit(&self, event: CoreEvent) {
        self.listener.on_event(event);
    }
}

pub struct AppCore {
    shared: Arc<Shared>,
    synth: Synthesizer,
    player: Player,
    auto_synthesize: bool,
}

impl AppCore {
    pub fn new(config: AppConfig, deps: AppDeps) -> Result<Self> {
        let audio_dir = config.data_dir.join("audio");
        std::fs::create_dir_all(&audio_dir)?;

        let voices = deps.tts.voices();
        let default_voice = config
            .default_voice
            .filter(|v| voices.iter().any(|x| &x.id == v))
            .or_else(|| voices.first().map(|v| v.id.clone()))
            .ok_or_else(|| CoreError::Engine("engine exposes no voices".into()))?;
        let cache_limit = deps
            .storage
            .get_setting(LIMIT_SETTING)?
            .and_then(|v| v.parse().ok())
            .unwrap_or(config.cache_limit_bytes);

        let shared = Arc::new(Shared {
            storage: deps.storage,
            tts: deps.tts,
            codec: deps.codec,
            listener: deps.listener,
            audio_dir,
            default_voice,
            cache_limit: AtomicU64::new(cache_limit),
            now_playing: Mutex::new(None),
            synthesizing: Mutex::new(None),
        });
        let player = Player::spawn(shared.clone(), deps.sink);
        let core = Self {
            synth: Synthesizer::spawn(shared.clone(), player.sender()),
            player,
            shared,
            auto_synthesize: config.auto_synthesize,
        };
        core.resume_interrupted()?;
        Ok(core)
    }

    /// 上次退出时排队中 / 合成中的文章重新入队，从断点继续。
    fn resume_interrupted(&self) -> Result<()> {
        for e in self.shared.storage.list_cache_entries()? {
            if matches!(e.state, AudioStatus::Queued | AudioStatus::Synthesizing) {
                self.shared
                    .storage
                    .set_cache_state(&e.article_id, AudioStatus::Queued, None)?;
                self.synth.enqueue(e.article_id, false);
            }
        }
        Ok(())
    }

    /// 把文章放进合成队列（已完整缓存则什么也不做）。
    fn queue(&self, id: &str, urgent: bool) -> Result<()> {
        let article = self.get_article(id)?;
        match self.shared.storage.get_cache_entry(id)? {
            Some(e) if e.state == AudioStatus::Ready => return Ok(()),
            Some(e) if e.state == AudioStatus::Synthesizing => {}
            Some(_) => self
                .shared
                .storage
                .set_cache_state(id, AudioStatus::Queued, None)?,
            None => {
                let voice = article
                    .voice_id
                    .clone()
                    .unwrap_or_else(|| self.shared.default_voice.clone());
                self.shared
                    .new_cache_entry(&article, &voice, AudioStatus::Queued)?;
            }
        }
        self.synth.enqueue(id.to_string(), urgent);
        self.shared.emit(CoreEvent::LibraryChanged);
        Ok(())
    }

    // ---------- 资料库 ----------

    /// 导入文章；开启自动合成时立即在后台开始转换。
    pub fn import_text(
        &self,
        title: Option<String>,
        body: String,
        source: SourceKind,
        source_ref: Option<String>,
    ) -> Result<ArticleSummary> {
        let plan = text::plan(&body);
        if plan.is_empty() {
            return Err(CoreError::InvalidInput("no readable English text".into()));
        }
        let title = title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| text::ingest::derive_title(&body));
        let sentences: Vec<SentenceSpan> = plan
            .iter()
            .enumerate()
            .map(|(i, s)| SentenceSpan {
                index: i as u32,
                paragraph: s.paragraph,
                text: s.text.clone(),
                ready: false,
                start_ms: 0,
                end_ms: 0,
                words: Vec::new(),
            })
            .collect();
        let article = Article {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            source,
            source_ref,
            body,
            created_at_ms: now_ms(),
            voice_id: None,
            pinned: false,
            sentence_count: sentences.len() as u32,
            estimated_ms: text::estimate_ms(&plan),
        };
        self.shared.storage.insert_article(&article, &sentences)?;
        if self.auto_synthesize {
            self.queue(&article.id, false)?;
        }
        self.shared.emit(CoreEvent::LibraryChanged);
        self.summary(&article.id)
    }

    pub fn list_articles(&self) -> Result<Vec<ArticleSummary>> {
        self.shared.storage.list_articles()
    }

    fn summary(&self, id: &str) -> Result<ArticleSummary> {
        self.list_articles()?
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| CoreError::NotFound(id.into()))
    }

    pub fn get_article(&self, id: &str) -> Result<Article> {
        self.shared
            .storage
            .get_article(id)?
            .ok_or_else(|| CoreError::NotFound(id.into()))
    }

    pub fn delete_article(&self, id: &str) -> Result<()> {
        self.synth.cancel(id);
        if self.player.state().article_id.as_deref() == Some(id) {
            self.player.send(PlayerCmd::Stop);
        }
        self.shared.remove_cache(id)?;
        self.shared.storage.delete_article(id)?;
        self.shared.emit(CoreEvent::LibraryChanged);
        Ok(())
    }

    // ---------- 分类 ----------

    pub fn list_collections(&self) -> Result<Vec<Collection>> {
        self.shared.storage.list_collections()
    }

    /// 新建分类。名称去掉首尾空白后不能为空、不能与已有分类重名（不区分大小写）。
    pub fn create_collection(&self, name: &str) -> Result<Collection> {
        let name = self.collection_name(name, None)?;
        let c = Collection {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            created_at_ms: now_ms(),
            article_ids: Vec::new(),
        };
        self.shared.storage.insert_collection(&c)?;
        self.shared.emit(CoreEvent::LibraryChanged);
        Ok(c)
    }

    pub fn rename_collection(&self, id: &str, name: &str) -> Result<()> {
        let name = self.collection_name(name, Some(id))?;
        self.shared.storage.rename_collection(id, &name)?;
        self.shared.emit(CoreEvent::LibraryChanged);
        Ok(())
    }

    /// 删除分类；其中的文章保留在资料库（只是不再属于它）。
    pub fn delete_collection(&self, id: &str) -> Result<()> {
        self.shared.storage.delete_collection(id)?;
        self.shared.emit(CoreEvent::LibraryChanged);
        Ok(())
    }

    /// 把文章加入分类（一篇可在多个分类里）；已在其中时不变。
    pub fn add_to_collection(&self, article_id: &str, collection_id: &str) -> Result<()> {
        self.shared
            .storage
            .add_to_collection(collection_id, article_id, now_ms())?;
        self.shared.emit(CoreEvent::LibraryChanged);
        Ok(())
    }

    /// 从分类中移除文章（文章本身保留）。
    pub fn remove_from_collection(&self, article_id: &str, collection_id: &str) -> Result<()> {
        self.shared
            .storage
            .remove_from_collection(collection_id, article_id)?;
        self.shared.emit(CoreEvent::LibraryChanged);
        Ok(())
    }

    fn collection_name(&self, name: &str, except: Option<&str>) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::InvalidInput("collection name is empty".into()));
        }
        let taken = self
            .list_collections()?
            .iter()
            .any(|c| Some(c.id.as_str()) != except && c.name.to_lowercase() == name.to_lowercase());
        if taken {
            return Err(CoreError::InvalidInput(format!(
                "collection \"{name}\" already exists"
            )));
        }
        Ok(name.to_string())
    }

    /// 完整原文时间轴：全部句子文本，已合成的带时间。
    pub fn timeline(&self, id: &str) -> Result<Timeline> {
        let complete = self
            .shared
            .storage
            .get_cache_entry(id)?
            .is_some_and(|e| e.state == AudioStatus::Ready);
        let sentences = self.shared.storage.list_sentences(id)?;
        Ok(Timeline {
            sentences: sentences.into_iter().map(|s| s.span).collect(),
            complete,
        })
    }

    // ---------- 合成 ----------

    pub fn voices(&self) -> Vec<Voice> {
        self.shared.tts.voices()
    }

    pub fn default_voice(&self) -> &str {
        &self.shared.default_voice
    }

    /// 开始/继续合成；传入新音色时丢弃旧缓存并重新合成。
    pub fn synthesize(&self, id: &str, voice_id: Option<String>) -> Result<()> {
        let article = self.get_article(id)?;
        let current = self.shared.storage.get_cache_entry(id)?;
        if let Some(voice) = voice_id {
            if !self.voices().iter().any(|v| v.id == voice) {
                return Err(CoreError::InvalidInput(format!("unknown voice {voice}")));
            }
            if current.as_ref().is_some_and(|e| e.voice_id != voice) {
                self.synth.cancel(id);
                self.shared.remove_cache(id)?;
            }
            self.shared.storage.set_article_voice(id, &voice)?;
        }
        let playing = self.player.state().article_id.as_deref() == Some(id);
        self.queue(&article.id, playing)?;
        if playing {
            // 换音色后重新加载新缓存，从当前位置继续
            let start_ms = self.player.state().position_ms;
            self.player.send(PlayerCmd::Load {
                article_id: article.id,
                start_ms,
                autoplay: true,
            });
        }
        Ok(())
    }

    /// 暂停合成，已合成部分保留，可随时继续。
    pub fn cancel_synthesis(&self, id: &str) {
        self.synth.cancel(id);
    }

    // ---------- 缓存池 ----------

    pub fn cache_stats(&self) -> Result<CacheStats> {
        self.shared.cache_stats()
    }

    pub fn set_cache_limit(&self, bytes: u64) -> Result<()> {
        self.shared
            .storage
            .set_setting(LIMIT_SETTING, &bytes.to_string())?;
        self.shared.cache_limit.store(bytes, Ordering::Relaxed);
        self.shared.enforce_cache_limit()?;
        self.emit_cache_changed();
        Ok(())
    }

    /// 钉住 =“保留下载”：不被淘汰，并确保完整合成。
    pub fn set_pinned(&self, id: &str, pinned: bool) -> Result<()> {
        self.shared.storage.set_pinned(id, pinned)?;
        if pinned {
            self.queue(id, false)?;
        } else {
            self.shared.enforce_cache_limit()?;
        }
        self.shared.emit(CoreEvent::LibraryChanged);
        self.emit_cache_changed();
        Ok(())
    }

    /// 删除一篇的缓存（“移除下载”），文章保留在资料库。正在播放时不允许。
    pub fn remove_download(&self, id: &str) -> Result<()> {
        if self.player.state().article_id.as_deref() == Some(id) {
            return Err(CoreError::InvalidInput(
                "article is currently playing".into(),
            ));
        }
        self.synth.cancel(id);
        self.shared.storage.set_pinned(id, false)?;
        self.shared.remove_cache(id)?;
        self.shared.emit(CoreEvent::LibraryChanged);
        self.emit_cache_changed();
        Ok(())
    }

    /// 清空所有未钉住、未在播放的缓存。返回清除的篇数。
    pub fn clear_cache(&self) -> Result<u32> {
        let playing = self.player.state().article_id;
        let mut cleared = 0;
        for e in self.shared.storage.eviction_candidates()? {
            if playing.as_ref() == Some(&e.article_id) {
                continue;
            }
            self.synth.cancel(&e.article_id);
            self.shared.remove_cache(&e.article_id)?;
            cleared += 1;
        }
        self.shared.emit(CoreEvent::LibraryChanged);
        self.emit_cache_changed();
        Ok(cleared)
    }

    fn emit_cache_changed(&self) {
        if let Ok(stats) = self.shared.cache_stats() {
            self.shared.emit(CoreEvent::CacheChanged { stats });
        }
    }

    // ---------- 播放 ----------

    /// 播放文章，从上次进度继续。未缓存完的文章会被提到合成队首，边听边转。
    pub fn play_article(&self, id: &str) -> Result<()> {
        self.queue(id, true)?;
        self.shared.storage.touch_cache_entry(id, now_ms())?;
        let start_ms = self.shared.storage.get_progress(id)?.unwrap_or(0);
        self.player.send(PlayerCmd::Load {
            article_id: id.to_string(),
            start_ms,
            autoplay: true,
        });
        Ok(())
    }

    pub fn play(&self) {
        self.player.send(PlayerCmd::Play);
    }

    pub fn pause(&self) {
        self.player.send(PlayerCmd::Pause);
    }

    pub fn toggle(&self) {
        match self.player.state().status {
            PlayerStatus::Playing | PlayerStatus::Buffering => self.pause(),
            _ => self.play(),
        }
    }

    pub fn seek(&self, position_ms: u64) {
        self.player.send(PlayerCmd::Seek(position_ms));
    }

    /// 相对跳转（如 ±15 秒）。
    pub fn skip(&self, delta_ms: i64) {
        let pos = self.player.state().position_ms as i64 + delta_ms;
        self.seek(pos.max(0) as u64);
    }

    /// 跳到第 `index` 句开头（点击原文、上一句/下一句）。该句须已合成。
    pub fn seek_sentence(&self, index: u32) -> Result<()> {
        let Some(id) = self.player.state().article_id else {
            return Ok(());
        };
        let start = self
            .timeline(&id)?
            .sentence_start_ms(index)
            .ok_or_else(|| {
                CoreError::InvalidInput(format!("sentence {index} is not synthesized yet"))
            })?;
        self.seek(start);
        Ok(())
    }

    pub fn set_rate(&self, rate: f32) {
        self.player.send(PlayerCmd::SetRate(rate));
    }

    pub fn stop(&self) {
        self.player.send(PlayerCmd::Stop);
    }

    pub fn player_state(&self) -> PlayerState {
        self.player.state()
    }
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
