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

/// 分类（类似歌单）：一篇文章可以同时在多个分类里。
/// 不叫 `Collection`：会与 Swift 的 `Collection` 协议、Kotlin 的 `kotlin.collections.Collection` 重名。
#[derive(uniffi::Record)]
pub struct ArticleCollection {
    pub id: String,
    pub name: String,
    pub created_at_ms: i64,
    /// 按加入时间先后（新加入的在后）。
    pub article_ids: Vec<String>,
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

// —— 翻译与查词 ——

/// 整句翻译引擎：Fast = 机器翻译（内置），Smart = 本地大模型（按需下载）。
#[derive(uniffi::Enum)]
pub enum TranslationEngine {
    Fast,
    Smart,
}

#[derive(uniffi::Record)]
pub struct SentenceTranslation {
    pub index: u32,
    pub text: String,
    pub engine: TranslationEngine,
}

#[derive(uniffi::Record)]
pub struct DictSense {
    /// `n.` / `vt.` / `[计]`；可为空。
    pub pos: String,
    pub meaning: String,
}

#[derive(uniffi::Record)]
pub struct WordForm {
    /// 过去式 / 复数 等。
    pub kind: String,
    pub word: String,
}

/// 离线词典查词结果。
#[derive(uniffi::Record)]
pub struct DictEntry {
    /// 词条原形。
    pub word: String,
    /// 用户点的词。
    pub query: String,
    /// `query` 是原形的哪种变形（“过去式”）；就是原形时为空。
    pub inflection: Option<String>,
    /// IPA（不含斜杠），可为空。
    pub phonetic: String,
    pub senses: Vec<DictSense>,
    /// 四级 / 六级 / 雅思 …
    pub tags: Vec<String>,
    pub collins: u8,
    pub oxford: bool,
    pub frequency: Option<u32>,
    pub forms: Vec<WordForm>,
}

#[derive(uniffi::Record)]
pub struct ExampleSentence {
    pub en: String,
    pub zh: String,
}

/// 点词“举一反三”（智能模式）。
#[derive(uniffi::Record)]
pub struct WordInsight {
    pub word: String,
    pub pos: String,
    /// 在这句里的意思。
    pub meaning: String,
    pub collocations: Vec<String>,
    pub examples: Vec<ExampleSentence>,
}

#[derive(uniffi::Record)]
pub struct GrammarPoint {
    pub text: String,
    pub note: String,
}

/// 句子解析（智能模式）。
#[derive(uniffi::Record)]
pub struct SentenceAnalysis {
    pub translation: String,
    pub structure: String,
    pub points: Vec<GrammarPoint>,
}

#[derive(uniffi::Record)]
pub struct GlossaryTerm {
    pub term: String,
    pub translation: String,
    pub created_at_ms: i64,
}

/// 生词本条目。文章删除后 `article_id` / `sentence_index` 为空，例句保留。
#[derive(uniffi::Record)]
pub struct VocabEntry {
    pub word: String,
    pub phonetic: String,
    pub meaning: String,
    pub article_id: Option<String>,
    pub article_title: Option<String>,
    pub sentence_index: Option<u32>,
    pub sentence: String,
    pub added_at_ms: i64,
}

#[derive(uniffi::Enum)]
pub enum LlmState {
    NotDownloaded,
    Downloading,
    Downloaded,
    Failed,
}

#[derive(uniffi::Record)]
pub struct LlmStatus {
    pub state: LlmState,
    pub model_name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub loaded: bool,
    pub error: Option<String>,
}

#[derive(uniffi::Record)]
pub struct TranslatorStatus {
    pub dictionary: bool,
    pub fast: bool,
    pub smart_enabled: bool,
    /// 整句翻译实际使用的引擎；都不可用时为空。
    pub active: Option<TranslationEngine>,
    pub llm: LlmStatus,
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
    /// 一句译文就绪（后台预翻 / 智能精翻）。
    TranslationProgress {
        article_id: String,
        translation: SentenceTranslation,
        translated: u32,
        total: u32,
    },
    /// 这篇译文被整体作废（术语表变化），需重新读取 `translations`。
    TranslationsChanged {
        article_id: String,
    },
    WordInsightReady {
        article_id: String,
        sentence: u32,
        word: u32,
        insight: Option<WordInsight>,
        error: Option<String>,
    },
    SentenceAnalysisReady {
        article_id: String,
        sentence: u32,
        analysis: Option<SentenceAnalysis>,
        error: Option<String>,
    },
    /// 翻译模式 / 模型下载进度 / 加载状态变化。
    TranslatorChanged {
        status: TranslatorStatus,
    },
    VocabularyChanged,
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

    // —— 分类 ——

    pub fn list_collections(&self) -> Result<Vec<ArticleCollection>> {
        Ok(self
            .inner
            .list_collections()?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// 名称去掉首尾空白后不能为空、不能与已有分类重名（不区分大小写）。
    pub fn create_collection(&self, name: String) -> Result<ArticleCollection> {
        Ok(self.inner.create_collection(&name)?.into())
    }

    pub fn rename_collection(&self, id: String, name: String) -> Result<()> {
        Ok(self.inner.rename_collection(&id, &name)?)
    }

    /// 删除分类；其中的文章保留。
    pub fn delete_collection(&self, id: String) -> Result<()> {
        Ok(self.inner.delete_collection(&id)?)
    }

    /// 已在其中时不变。
    pub fn add_to_collection(&self, article_id: String, collection_id: String) -> Result<()> {
        Ok(self.inner.add_to_collection(&article_id, &collection_id)?)
    }

    pub fn remove_from_collection(&self, article_id: String, collection_id: String) -> Result<()> {
        Ok(self
            .inner
            .remove_from_collection(&article_id, &collection_id)?)
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

    // —— 翻译与查词 ——

    pub fn translator_status(&self) -> TranslatorStatus {
        self.inner.translator_status().into()
    }

    /// 每句最多一条（智能精翻优先）；缺的句子之后经 `TranslationProgress` 送达。
    pub fn translations(&self, id: String) -> Result<Vec<SentenceTranslation>> {
        Ok(self
            .inner
            .translations(&id)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// 用户正在看这篇（打开播放页 / 打开双语）：优先翻译，智能模式下逐句精翻。
    pub fn prepare_translations(&self, id: String) -> Result<()> {
        Ok(self.inner.prepare_translations(&id)?)
    }

    /// 离线词典查词（毫秒级）。
    pub fn lookup_word(&self, word: String) -> Result<Option<DictEntry>> {
        Ok(self.inner.lookup_word(&word)?.map(Into::into))
    }

    /// 有缓存直接返回；否则返回空并在后台生成（`WordInsightReady`）。智能模式不可用时报错。
    pub fn word_insight(
        &self,
        id: String,
        sentence: u32,
        word: u32,
    ) -> Result<Option<WordInsight>> {
        Ok(self
            .inner
            .word_insight(&id, sentence, word)?
            .map(Into::into))
    }

    /// 用法同 `word_insight`，结果经 `SentenceAnalysisReady` 送达。
    pub fn sentence_analysis(&self, id: String, sentence: u32) -> Result<Option<SentenceAnalysis>> {
        Ok(self.inner.sentence_analysis(&id, sentence)?.map(Into::into))
    }

    pub fn set_smart_translation(&self, enabled: bool) -> Result<()> {
        Ok(self.inner.set_smart_translation(enabled)?)
    }

    /// 后台下载智能翻译模型（断点续传）。是否在蜂窝网络下载由平台判断并提示用户。
    pub fn download_llm(&self) -> Result<()> {
        Ok(self.inner.download_llm()?)
    }

    pub fn cancel_llm_download(&self) {
        self.inner.cancel_llm_download()
    }

    pub fn delete_llm(&self) -> Result<()> {
        Ok(self.inner.delete_llm()?)
    }

    pub fn list_glossary(&self) -> Result<Vec<GlossaryTerm>> {
        Ok(self
            .inner
            .list_glossary()?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// 添加或修改术语（不区分大小写）；含该术语的句子会重新翻译。
    pub fn add_glossary_term(&self, term: String, translation: String) -> Result<GlossaryTerm> {
        Ok(self.inner.add_glossary_term(&term, &translation)?.into())
    }

    pub fn remove_glossary_term(&self, term: String) -> Result<()> {
        Ok(self.inner.remove_glossary_term(&term)?)
    }

    pub fn list_vocab(&self) -> Result<Vec<VocabEntry>> {
        Ok(self
            .inner
            .list_vocab()?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// 收藏第 `sentence` 句第 `word` 个词（按原形去重）。
    pub fn add_vocab(&self, id: String, sentence: u32, word: u32) -> Result<VocabEntry> {
        Ok(self.inner.add_vocab(&id, sentence, word)?.into())
    }

    pub fn remove_vocab(&self, word: String) -> Result<()> {
        Ok(self.inner.remove_vocab(&word)?)
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
            CoreEvent::TranslationProgress {
                article_id,
                translation,
                translated,
                total,
            } => SpeakerEvent::TranslationProgress {
                article_id,
                translation: translation.into(),
                translated,
                total,
            },
            CoreEvent::TranslationsChanged { article_id } => {
                SpeakerEvent::TranslationsChanged { article_id }
            }
            CoreEvent::WordInsightReady {
                article_id,
                sentence,
                word,
                insight,
                error,
            } => SpeakerEvent::WordInsightReady {
                article_id,
                sentence,
                word,
                insight: insight.map(Into::into),
                error,
            },
            CoreEvent::SentenceAnalysisReady {
                article_id,
                sentence,
                analysis,
                error,
            } => SpeakerEvent::SentenceAnalysisReady {
                article_id,
                sentence,
                analysis: analysis.map(Into::into),
                error,
            },
            CoreEvent::TranslatorChanged { status } => SpeakerEvent::TranslatorChanged {
                status: status.into(),
            },
            CoreEvent::VocabularyChanged => SpeakerEvent::VocabularyChanged,
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

impl From<m::Collection> for ArticleCollection {
    fn from(c: m::Collection) -> Self {
        Self {
            id: c.id,
            name: c.name,
            created_at_ms: c.created_at_ms,
            article_ids: c.article_ids,
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

impl From<m::TranslationEngine> for TranslationEngine {
    fn from(e: m::TranslationEngine) -> Self {
        match e {
            m::TranslationEngine::Fast => Self::Fast,
            m::TranslationEngine::Smart => Self::Smart,
        }
    }
}

impl From<m::SentenceTranslation> for SentenceTranslation {
    fn from(t: m::SentenceTranslation) -> Self {
        Self {
            index: t.index,
            text: t.text,
            engine: t.engine.into(),
        }
    }
}

impl From<m::DictEntry> for DictEntry {
    fn from(d: m::DictEntry) -> Self {
        Self {
            word: d.word,
            query: d.query,
            inflection: d.inflection,
            phonetic: d.phonetic,
            senses: d
                .senses
                .into_iter()
                .map(|s| DictSense {
                    pos: s.pos,
                    meaning: s.meaning,
                })
                .collect(),
            tags: d.tags,
            collins: d.collins,
            oxford: d.oxford,
            frequency: d.frequency,
            forms: d
                .forms
                .into_iter()
                .map(|f| WordForm {
                    kind: f.kind,
                    word: f.word,
                })
                .collect(),
        }
    }
}

impl From<m::WordInsight> for WordInsight {
    fn from(w: m::WordInsight) -> Self {
        Self {
            word: w.word,
            pos: w.pos,
            meaning: w.meaning,
            collocations: w.collocations,
            examples: w
                .examples
                .into_iter()
                .map(|e| ExampleSentence { en: e.en, zh: e.zh })
                .collect(),
        }
    }
}

impl From<m::SentenceAnalysis> for SentenceAnalysis {
    fn from(a: m::SentenceAnalysis) -> Self {
        Self {
            translation: a.translation,
            structure: a.structure,
            points: a
                .points
                .into_iter()
                .map(|p| GrammarPoint {
                    text: p.text,
                    note: p.note,
                })
                .collect(),
        }
    }
}

impl From<m::GlossaryTerm> for GlossaryTerm {
    fn from(t: m::GlossaryTerm) -> Self {
        Self {
            term: t.term,
            translation: t.translation,
            created_at_ms: t.created_at_ms,
        }
    }
}

impl From<m::VocabEntry> for VocabEntry {
    fn from(v: m::VocabEntry) -> Self {
        Self {
            word: v.word,
            phonetic: v.phonetic,
            meaning: v.meaning,
            article_id: v.article_id,
            article_title: v.article_title,
            sentence_index: v.sentence_index,
            sentence: v.sentence,
            added_at_ms: v.added_at_ms,
        }
    }
}

impl From<m::TranslatorStatus> for TranslatorStatus {
    fn from(s: m::TranslatorStatus) -> Self {
        Self {
            dictionary: s.dictionary,
            fast: s.fast,
            smart_enabled: s.smart_enabled,
            active: s.active.map(Into::into),
            llm: LlmStatus {
                state: match s.llm.state {
                    m::LlmState::NotDownloaded => LlmState::NotDownloaded,
                    m::LlmState::Downloading => LlmState::Downloading,
                    m::LlmState::Downloaded => LlmState::Downloaded,
                    m::LlmState::Failed => LlmState::Failed,
                },
                model_name: s.llm.model_name,
                downloaded_bytes: s.llm.downloaded_bytes,
                total_bytes: s.llm.total_bytes,
                loaded: s.llm.loaded,
                error: s.llm.error,
            },
        }
    }
}
