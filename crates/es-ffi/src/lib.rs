//! 移动端 FFI（UniFFI → Swift / Kotlin）。
//!
//! 这里只做“类型翻译 + 转发”，不放业务逻辑。DTO 与 `es-core::model` 一一对应，
//! 单独定义是为了让核心层不依赖 UniFFI，并能自由演进内部结构。

use std::path::PathBuf;
use std::sync::Arc;

use es_runtime::es_core::model as m;
use es_runtime::es_core::ports;
use es_runtime::es_core::text::connected;
use es_runtime::es_core::{self as core, AppCore, CoreEvent};
use es_runtime::RuntimeConfig;

uniffi::setup_scaffolding!();

// ---------- 错误 ----------

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum SpeakerError {
    #[error("not found: {message}")]
    NotFound { message: String },
    #[error("invalid input: {message}")]
    InvalidInput { message: String },
    #[error("{message}")]
    Failure { message: String },
}

impl From<core::CoreError> for SpeakerError {
    fn from(e: core::CoreError) -> Self {
        match e {
            core::CoreError::NotFound(message) => Self::NotFound { message },
            core::CoreError::InvalidInput(message) => Self::InvalidInput { message },
            other => Self::Failure {
                message: other.to_string(),
            },
        }
    }
}

type Result<T> = std::result::Result<T, SpeakerError>;

// ---------- DTO ----------

#[derive(uniffi::Enum)]
pub enum SourceKind {
    Paste,
    File,
    Share,
}

/// 音频缓存状态（类似“下载状态”）。
#[derive(uniffi::Enum)]
pub enum AudioStatus {
    NotCached,
    Queued,
    Synthesizing,
    Partial,
    Ready,
    Failed,
}

#[derive(uniffi::Enum)]
pub enum Accent {
    American,
    British,
}

#[derive(uniffi::Enum)]
pub enum Gender {
    Female,
    Male,
}

#[derive(uniffi::Enum)]
pub enum PlayerStatus {
    Idle,
    Playing,
    Buffering,
    Paused,
    Ended,
}

#[derive(uniffi::Record)]
pub struct ArticleSummary {
    pub id: String,
    pub title: String,
    pub source: SourceKind,
    pub source_ref: Option<String>,
    pub created_at_ms: i64,
    pub voice_id: Option<String>,
    pub pinned: bool,
    pub status: AudioStatus,
    pub ready_sentences: u32,
    pub total_sentences: u32,
    pub cached_bytes: u64,
    pub duration_ms: u64,
    pub progress_ms: Option<u64>,
}

#[derive(uniffi::Record)]
pub struct Article {
    pub id: String,
    pub title: String,
    pub body: String,
    pub voice_id: Option<String>,
    pub pinned: bool,
}

#[derive(uniffi::Record)]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub accent: Accent,
    pub gender: Gender,
}

/// 与下一个词的连读方式（跟读标注）。
#[derive(uniffi::Enum)]
pub enum Link {
    /// 辅音 + 元音：pick‿it
    ConsonantVowel,
    /// 元音间带出 /j/：see‿it
    GlideJ,
    /// 元音间带出 /w/：go‿on
    GlideW,
    /// 英式连接 r：far‿away
    LinkingR,
    /// 相同辅音合并：big‿game
    SameSound,
    /// 美式跨词闪音：get‿it
    Flap,
    /// 同化：did‿you → /dɪʤu/
    Assimilation,
}

#[derive(uniffi::Record)]
pub struct WordSpan {
    pub text: String,
    pub char_start: u32,
    pub char_end: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    /// 词典读音（IPA）；引擎不提供音素时为空。
    pub ipa: String,
    /// 与下一个词的连读，UI 在两词之间画 ‿。
    pub link: Option<Link>,
    /// 弱读形式（IPA），如 and → ən；UI 可淡化显示。
    pub weak_form: Option<String>,
    /// 词内美式闪音（water、better）。
    pub flap: bool,
    /// 词尾 t/d 在辅音前省略（next day）。
    pub elided: bool,
}

#[derive(uniffi::Record)]
pub struct SentenceSpan {
    pub index: u32,
    pub paragraph: u32,
    pub text: String,
    pub ready: bool,
    pub start_ms: u64,
    pub end_ms: u64,
    pub words: Vec<WordSpan>,
}

#[derive(uniffi::Record)]
pub struct Timeline {
    pub sentences: Vec<SentenceSpan>,
    pub complete: bool,
}

#[derive(uniffi::Record)]
pub struct PlayerState {
    pub article_id: Option<String>,
    pub status: PlayerStatus,
    pub position_ms: u64,
    pub buffered_ms: u64,
    pub duration_ms: u64,
    pub duration_final: bool,
    pub rate: f32,
}

#[derive(uniffi::Record)]
pub struct CacheStats {
    pub used_bytes: u64,
    pub limit_bytes: u64,
    pub pinned_bytes: u64,
    pub entries: u32,
}

#[derive(uniffi::Enum)]
pub enum SpeakerEvent {
    LibraryChanged,
    SynthesisProgress {
        article_id: String,
        ready: u32,
        total: u32,
        sentence: SentenceSpan,
    },
    SynthesisFinished {
        article_id: String,
        duration_ms: u64,
    },
    SynthesisFailed {
        article_id: String,
        message: String,
    },
    CacheChanged {
        stats: CacheStats,
    },
    PlayerStateChanged {
        state: PlayerState,
    },
    PlayerPosition {
        position_ms: u64,
        buffered_ms: u64,
        sentence: Option<u32>,
        word: Option<u32>,
    },
}

// ---------- 平台实现的回调接口 ----------

/// 平台音频输出。语义见 `es_core::ports::AudioSink`：
/// `write` 需在内部缓冲超过约 200ms 时阻塞。
#[uniffi::export(with_foreign)]
pub trait PlatformAudioSink: Send + Sync {
    // 避开 Swift 关键字 open：UniFFI 0.32 会把它转义进 C 头文件导致编译失败。
    fn open_output(&self, sample_rate: u32, channels: u16) -> bool;
    fn write(&self, samples: Vec<f32>);
    fn pause(&self);
    fn resume(&self);
    fn flush(&self);
    fn played_frames(&self) -> u64;
    fn set_rate(&self, rate: f32) -> bool;
    fn close_output(&self);
}

/// 事件回调。可能在后台线程调用，平台需切回主线程更新 UI。
#[uniffi::export(with_foreign)]
pub trait SpeakerListener: Send + Sync {
    fn on_event(&self, event: SpeakerEvent);
}

// ---------- 门面对象 ----------

#[derive(uniffi::Object)]
pub struct SpeakerCore {
    inner: AppCore,
}

#[uniffi::export]
impl SpeakerCore {
    /// `data_dir`：可写的应用数据目录；`model_dir`：随包分发的只读模型目录。
    #[uniffi::constructor]
    pub fn new(
        data_dir: String,
        model_dir: Option<String>,
        default_voice: Option<String>,
        sink: Arc<dyn PlatformAudioSink>,
        listener: Arc<dyn SpeakerListener>,
    ) -> Result<Arc<Self>> {
        let mut config = RuntimeConfig::new(PathBuf::from(data_dir));
        config.model_dir = model_dir.map(PathBuf::from);
        config.default_voice = default_voice;
        let inner = es_runtime::build(
            config,
            Arc::new(SinkAdapter(sink)),
            Arc::new(ListenerAdapter(listener)),
        )?;
        Ok(Arc::new(Self { inner }))
    }

    // —— 资料库 ——

    /// 导入后立即在后台开始转换。
    pub fn import_text(
        &self,
        title: Option<String>,
        body: String,
        source: SourceKind,
        source_ref: Option<String>,
    ) -> Result<ArticleSummary> {
        let source = match source {
            SourceKind::Paste => m::SourceKind::Paste,
            SourceKind::File => m::SourceKind::File,
            SourceKind::Share => m::SourceKind::Share,
        };
        Ok(self
            .inner
            .import_text(title, body, source, source_ref)?
            .into())
    }

    pub fn list_articles(&self) -> Result<Vec<ArticleSummary>> {
        Ok(self
            .inner
            .list_articles()?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn get_article(&self, id: String) -> Result<Article> {
        let a = self.inner.get_article(&id)?;
        Ok(Article {
            id: a.id,
            title: a.title,
            body: a.body,
            voice_id: a.voice_id,
            pinned: a.pinned,
        })
    }

    pub fn delete_article(&self, id: String) -> Result<()> {
        Ok(self.inner.delete_article(&id)?)
    }

    /// 全部句子文本；已合成的句子带时间（`ready = true`）。
    pub fn timeline(&self, id: String) -> Result<Timeline> {
        let t = self.inner.timeline(&id)?;
        Ok(Timeline {
            sentences: t.sentences.into_iter().map(Into::into).collect(),
            complete: t.complete,
        })
    }

    // —— 合成 ——

    pub fn voices(&self) -> Vec<Voice> {
        self.inner.voices().into_iter().map(Into::into).collect()
    }

    pub fn synthesize(&self, id: String, voice_id: Option<String>) -> Result<()> {
        Ok(self.inner.synthesize(&id, voice_id)?)
    }

    pub fn cancel_synthesis(&self, id: String) {
        self.inner.cancel_synthesis(&id)
    }

    // —— 缓存池 ——

    pub fn cache_stats(&self) -> Result<CacheStats> {
        Ok(self.inner.cache_stats()?.into())
    }

    pub fn set_cache_limit(&self, bytes: u64) -> Result<()> {
        Ok(self.inner.set_cache_limit(bytes)?)
    }

    /// 钉住 =“保留下载”：不被淘汰，并确保完整合成。
    pub fn set_pinned(&self, id: String, pinned: bool) -> Result<()> {
        Ok(self.inner.set_pinned(&id, pinned)?)
    }

    pub fn remove_download(&self, id: String) -> Result<()> {
        Ok(self.inner.remove_download(&id)?)
    }

    pub fn clear_cache(&self) -> Result<u32> {
        Ok(self.inner.clear_cache()?)
    }

    // —— 播放 ——

    /// 未缓存完的文章会被提到合成队首，边听边转。
    pub fn play_article(&self, id: String) -> Result<()> {
        Ok(self.inner.play_article(&id)?)
    }

    pub fn play(&self) {
        self.inner.play()
    }

    pub fn pause(&self) {
        self.inner.pause()
    }

    pub fn toggle(&self) {
        self.inner.toggle()
    }

    pub fn seek(&self, position_ms: u64) {
        self.inner.seek(position_ms)
    }

    pub fn skip(&self, delta_ms: i64) {
        self.inner.skip(delta_ms)
    }

    pub fn seek_sentence(&self, index: u32) -> Result<()> {
        Ok(self.inner.seek_sentence(index)?)
    }

    pub fn set_rate(&self, rate: f32) {
        self.inner.set_rate(rate)
    }

    pub fn stop(&self) {
        self.inner.stop()
    }

    pub fn player_state(&self) -> PlayerState {
        self.inner.player_state().into()
    }
}

// ---------- 适配器 ----------

struct SinkAdapter(Arc<dyn PlatformAudioSink>);

impl ports::AudioSink for SinkAdapter {
    fn open(&self, f: m::PcmFormat) -> core::Result<()> {
        if self.0.open_output(f.sample_rate, f.channels) {
            Ok(())
        } else {
            Err(core::CoreError::Audio(
                "platform audio sink failed to open".into(),
            ))
        }
    }
    fn write(&self, samples: &[f32]) -> core::Result<()> {
        self.0.write(samples.to_vec());
        Ok(())
    }
    fn pause(&self) {
        self.0.pause()
    }
    fn resume(&self) {
        self.0.resume()
    }
    fn flush(&self) {
        self.0.flush()
    }
    fn played_frames(&self) -> u64 {
        self.0.played_frames()
    }
    fn set_rate(&self, rate: f32) -> bool {
        self.0.set_rate(rate)
    }
    fn close(&self) {
        self.0.close_output()
    }
}

struct ListenerAdapter(Arc<dyn SpeakerListener>);

impl core::EventListener for ListenerAdapter {
    fn on_event(&self, e: CoreEvent) {
        let event = match e {
            CoreEvent::LibraryChanged => SpeakerEvent::LibraryChanged,
            CoreEvent::SynthesisProgress {
                article_id,
                ready,
                total,
                sentence,
            } => SpeakerEvent::SynthesisProgress {
                article_id,
                ready,
                total,
                sentence: sentence.into(),
            },
            CoreEvent::SynthesisFinished {
                article_id,
                duration_ms,
            } => SpeakerEvent::SynthesisFinished {
                article_id,
                duration_ms,
            },
            CoreEvent::SynthesisFailed {
                article_id,
                message,
            } => SpeakerEvent::SynthesisFailed {
                article_id,
                message,
            },
            CoreEvent::CacheChanged { stats } => SpeakerEvent::CacheChanged {
                stats: stats.into(),
            },
            CoreEvent::PlayerStateChanged { state } => SpeakerEvent::PlayerStateChanged {
                state: state.into(),
            },
            CoreEvent::PlayerPosition {
                position_ms,
                buffered_ms,
                sentence,
                word,
            } => SpeakerEvent::PlayerPosition {
                position_ms,
                buffered_ms,
                sentence,
                word,
            },
        };
        self.0.on_event(event);
    }
}

// ---------- 类型转换 ----------

impl From<m::AudioStatus> for AudioStatus {
    fn from(s: m::AudioStatus) -> Self {
        match s {
            m::AudioStatus::NotCached => Self::NotCached,
            m::AudioStatus::Queued => Self::Queued,
            m::AudioStatus::Synthesizing => Self::Synthesizing,
            m::AudioStatus::Partial => Self::Partial,
            m::AudioStatus::Ready => Self::Ready,
            m::AudioStatus::Failed => Self::Failed,
        }
    }
}

impl From<m::SourceKind> for SourceKind {
    fn from(s: m::SourceKind) -> Self {
        match s {
            m::SourceKind::Paste => Self::Paste,
            m::SourceKind::File => Self::File,
            m::SourceKind::Share => Self::Share,
        }
    }
}

impl From<m::ArticleSummary> for ArticleSummary {
    fn from(a: m::ArticleSummary) -> Self {
        Self {
            id: a.id,
            title: a.title,
            source: a.source.into(),
            source_ref: a.source_ref,
            created_at_ms: a.created_at_ms,
            voice_id: a.voice_id,
            pinned: a.pinned,
            status: a.status.into(),
            ready_sentences: a.ready_sentences,
            total_sentences: a.total_sentences,
            cached_bytes: a.cached_bytes,
            duration_ms: a.duration_ms,
            progress_ms: a.progress_ms,
        }
    }
}

impl From<m::Voice> for Voice {
    fn from(v: m::Voice) -> Self {
        Self {
            id: v.id,
            name: v.name,
            accent: match v.accent {
                m::Accent::American => Accent::American,
                m::Accent::British => Accent::British,
            },
            gender: match v.gender {
                m::Gender::Female => Gender::Female,
                m::Gender::Male => Gender::Male,
            },
        }
    }
}

impl From<m::SentenceSpan> for SentenceSpan {
    fn from(s: m::SentenceSpan) -> Self {
        Self {
            index: s.index,
            paragraph: s.paragraph,
            text: s.text,
            ready: s.ready,
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            words: s
                .words
                .into_iter()
                .map(|w| WordSpan {
                    text: w.text,
                    char_start: w.char_start,
                    char_end: w.char_end,
                    start_ms: w.start_ms,
                    end_ms: w.end_ms,
                    ipa: w.notes.ipa,
                    link: w.notes.link.map(Into::into),
                    weak_form: w.notes.weak_form,
                    flap: w.notes.flap,
                    elided: w.notes.elided,
                })
                .collect(),
        }
    }
}

impl From<connected::Link> for Link {
    fn from(l: connected::Link) -> Self {
        match l {
            connected::Link::ConsonantVowel => Self::ConsonantVowel,
            connected::Link::GlideJ => Self::GlideJ,
            connected::Link::GlideW => Self::GlideW,
            connected::Link::LinkingR => Self::LinkingR,
            connected::Link::SameSound => Self::SameSound,
            connected::Link::Flap => Self::Flap,
            connected::Link::Assimilation => Self::Assimilation,
        }
    }
}

impl From<m::CacheStats> for CacheStats {
    fn from(s: m::CacheStats) -> Self {
        Self {
            used_bytes: s.used_bytes,
            limit_bytes: s.limit_bytes,
            pinned_bytes: s.pinned_bytes,
            entries: s.entries,
        }
    }
}

impl From<m::PlayerState> for PlayerState {
    fn from(s: m::PlayerState) -> Self {
        Self {
            article_id: s.article_id,
            status: match s.status {
                m::PlayerStatus::Idle => PlayerStatus::Idle,
                m::PlayerStatus::Playing => PlayerStatus::Playing,
                m::PlayerStatus::Buffering => PlayerStatus::Buffering,
                m::PlayerStatus::Paused => PlayerStatus::Paused,
                m::PlayerStatus::Ended => PlayerStatus::Ended,
            },
            position_ms: s.position_ms,
            buffered_ms: s.buffered_ms,
            duration_ms: s.duration_ms,
            duration_final: s.duration_final,
            rate: s.rate,
        }
    }
}
