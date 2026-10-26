//! 核心层对外依赖的抽象（端口）。
//!
//! | trait          | 由谁实现                                   |
//! |----------------|--------------------------------------------|
//! | `TtsEngine`    | `es-tts-kokoro`（ONNX 推理）               |
//! | `Storage`      | `es-storage`（SQLite）                     |
//! | `AudioCodec`   | `es-audio`（PCM s16，后续 Opus）           |
//! | `AudioSink`    | 平台层：iOS AVAudioEngine / Android AudioTrack / 桌面 cpal |
//! | `EventListener`| 平台层：UI 状态更新                        |
//! | `Translator`   | `es-translate`（Opus-MT，ONNX）            |
//! | `Dictionary`   | `es-translate`（ECDICT，SQLite）           |
//! | `LlmProvider` / `LanguageModel` | `es-translate`（Qwen，llama.cpp）|

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::error::Result;
use crate::model::*;

// ---------- TTS ----------

pub struct SynthRequest<'a> {
    /// 规范化后的可朗读文本（一个句子或句子片段）。
    pub text: &'a str,
    /// 与 `text` 对应的逐词朗读形式，引擎若能对齐可据此返回逐词时长。
    pub words: &'a [String],
    pub voice_id: &'a str,
    pub speed: f32,
}

pub struct SynthOutput {
    /// 单声道 f32 PCM，采样率为 `TtsEngine::sample_rate()`。
    pub samples: Vec<f32>,
    /// 每个词的 (起始帧, 结束帧)，长度与 `SynthRequest::words` 相同。
    /// 引擎无法提供时为 `None`，核心层按字符数估算。
    pub word_frames: Option<Vec<(u64, u64)>>,
    /// 每个词的音素（misaki / Kokoro 记号，见 `text::connected`），用于读音与连读标注。
    /// 长度与 `SynthRequest::words` 相同；引擎无法提供时为 `None`。
    pub word_phonemes: Option<Vec<String>>,
}

pub trait TtsEngine: Send + Sync {
    fn id(&self) -> &str;
    fn sample_rate(&self) -> u32;
    fn voices(&self) -> Vec<Voice>;
    fn synthesize(&self, req: &SynthRequest<'_>) -> Result<SynthOutput>;
}

// ---------- 存储 ----------

/// 持久化：资料库 + 缓存池索引 + 播放进度 + 设置。
///
/// 缓存池的音频数据在文件里，**所有索引都在这里**：每句的块位置、
/// 每篇的缓存状态/大小/最近访问时间。崩溃恢复和 LRU 淘汰都只依赖这些记录。
pub trait Storage: Send + Sync {
    // —— 资料库 ——
    /// 插入文章及其全部（未合成的）句子。
    fn insert_article(&self, article: &Article, sentences: &[SentenceSpan]) -> Result<()>;
    fn get_article(&self, id: &str) -> Result<Option<Article>>;
    fn list_articles(&self) -> Result<Vec<ArticleSummary>>;
    /// 级联删除句子、缓存记录与进度（音频文件由调用方删除）。
    fn delete_article(&self, id: &str) -> Result<()>;
    fn set_article_voice(&self, id: &str, voice_id: &str) -> Result<()>;
    fn set_pinned(&self, id: &str, pinned: bool) -> Result<()>;
    fn list_sentences(&self, article_id: &str) -> Result<Vec<SentenceRecord>>;

    // —— 分类 ——
    /// 按创建时间升序，各自带上文章（按加入时间升序）。
    fn list_collections(&self) -> Result<Vec<Collection>>;
    /// 只写入分类本身（`article_ids` 忽略）。
    fn insert_collection(&self, collection: &Collection) -> Result<()>;
    fn rename_collection(&self, id: &str, name: &str) -> Result<()>;
    /// 删除分类；文章本身保留（只是不再属于它）。
    fn delete_collection(&self, id: &str) -> Result<()>;
    /// 把文章加入分类；已在其中时不变（保留原加入时间）。分类或文章不存在时返回 `NotFound`。
    fn add_to_collection(
        &self,
        collection_id: &str,
        article_id: &str,
        added_at_ms: i64,
    ) -> Result<()>;
    fn remove_from_collection(&self, collection_id: &str, article_id: &str) -> Result<()>;

    // —— 缓存池 ——
    fn get_cache_entry(&self, article_id: &str) -> Result<Option<CacheEntry>>;
    fn list_cache_entries(&self) -> Result<Vec<CacheEntry>>;
    /// 新建（或替换）缓存记录，并清空该文章所有句子的块信息。
    fn create_cache_entry(&self, entry: &CacheEntry) -> Result<()>;
    fn set_cache_state(
        &self,
        article_id: &str,
        state: AudioStatus,
        error: Option<&str>,
    ) -> Result<()>;
    /// 原子地提交一句：写入块位置与时间轴，并累加缓存记录的句数/帧数/字节数。
    /// 仅当 `entry.file_name` 仍是当前记录时生效（防止换音色后旧任务写入新缓存），
    /// 否则返回 `NotFound`。
    fn commit_chunk(&self, entry: &CacheEntry, chunk: &Chunk, span: &SentenceSpan) -> Result<()>;
    fn touch_cache_entry(&self, article_id: &str, now_ms: i64) -> Result<()>;
    /// 删除缓存记录并清空块信息（文章保留在资料库中）。
    fn delete_cache_entry(&self, article_id: &str) -> Result<()>;
    /// 可淘汰的记录：未钉住、有数据，按最近访问时间升序（LRU）。
    fn eviction_candidates(&self) -> Result<Vec<CacheEntry>>;
    /// (总字节数, 钉住的字节数, 记录数)
    fn cache_usage(&self) -> Result<(u64, u64, u32)>;

    // —— 进度 / 设置 ——
    fn save_progress(&self, article_id: &str, position_ms: u64) -> Result<()>;
    fn get_progress(&self, article_id: &str) -> Result<Option<u64>>;
    fn get_setting(&self, key: &str) -> Result<Option<String>>;
    fn set_setting(&self, key: &str, value: &str) -> Result<()>;

    // —— 译文缓存 ——
    /// 写入（或替换）一句某个引擎的译文。
    fn put_translation(
        &self,
        article_id: &str,
        index: u32,
        engine: TranslationEngine,
        text: &str,
    ) -> Result<()>;
    /// 一篇的全部译文（同一句可能有 Fast 与 Smart 两条），按句序。
    fn list_translations(&self, article_id: &str) -> Result<Vec<SentenceTranslation>>;
    /// 删除原文包含 `term`（不区分大小写）的句子的译文；术语表变化后调用，返回受影响的文章。
    fn invalidate_translations(&self, term: &str) -> Result<Vec<ArticleId>>;

    // —— 智能解析缓存（点词举一反三 / 句子解析，JSON） ——
    /// `word` 为空表示整句解析。
    fn get_ai_note(
        &self,
        article_id: &str,
        sentence: u32,
        word: Option<u32>,
    ) -> Result<Option<String>>;
    fn put_ai_note(
        &self,
        article_id: &str,
        sentence: u32,
        word: Option<u32>,
        json: &str,
    ) -> Result<()>;

    // —— 术语表 ——
    fn list_glossary(&self) -> Result<Vec<GlossaryTerm>>;
    /// 按术语（不区分大小写）插入或替换。
    fn upsert_glossary(&self, term: &GlossaryTerm) -> Result<()>;
    fn delete_glossary(&self, term: &str) -> Result<()>;

    // —— 生词本 ——
    /// 按添加时间倒序（新收藏的在前）。
    fn list_vocab(&self) -> Result<Vec<VocabEntry>>;
    /// 按 `word` 插入或替换（再次收藏时更新出处与时间）。
    fn upsert_vocab(&self, entry: &VocabEntry) -> Result<()>;
    fn delete_vocab(&self, word: &str) -> Result<()>;
}

// ---------- 编解码 ----------

/// 以“块”为单位编解码：每句话独立编码，可单独解码。
/// 这样缓存文件可以边写边读，断点续转只需截断到最后一个完整块。
pub trait AudioCodec: Send + Sync {
    /// 写入缓存记录，用于识别旧格式。
    fn id(&self) -> &str;
    fn extension(&self) -> &str;
    fn encode(&self, pcm: &[f32], format: PcmFormat) -> Result<Vec<u8>>;
    fn decode(&self, data: &[u8], format: PcmFormat) -> Result<Vec<f32>>;
}

// ---------- 音频输出（平台实现） ----------

/// 平台音频输出。核心层的播放线程持续推送 PCM。
///
/// 约定：
/// - `open` 之后设备处于播放状态，缓冲为空时输出静音；
/// - `write` 在内部缓冲超过约 200ms 时**阻塞**，以此形成背压；
/// - `played_frames` 返回自上次 `open`/`flush` 以来设备实际播放的帧数（按源采样率计）；
/// - 变速（保持音调）由平台实现：桌面端为内置 WSOLA，移动端可用 AVAudioUnitTimePitch。
pub trait AudioSink: Send + Sync {
    fn open(&self, format: PcmFormat) -> Result<()>;
    fn write(&self, samples: &[f32]) -> Result<()>;
    fn pause(&self);
    fn resume(&self);
    /// 丢弃未播放的缓冲，并把 `played_frames` 归零。
    fn flush(&self);
    fn played_frames(&self) -> u64;
    /// 返回平台是否支持该倍速。
    fn set_rate(&self, rate: f32) -> bool;
    /// 输入流结束（之后不会再有 `write`）：冲出变速/缓冲里残留的尾部，
    /// 让 `played_frames` 能最终追上已写入的帧数（否则结尾会被截断）。
    fn end_stream(&self) {}
    fn close(&self);
}

// ---------- 翻译与查词 ----------

/// 机器翻译（英 → 中），一次一句。实现须线程安全，可能在后台线程调用。
pub trait Translator: Send + Sync {
    fn id(&self) -> &str;
    fn translate(&self, text: &str) -> Result<String>;
}

/// 离线词典。`word` 可以是变形（perceived），实现负责还原到原形。
pub trait Dictionary: Send + Sync {
    fn lookup(&self, word: &str) -> Result<Option<DictEntry>>;
}

pub struct LlmRequest<'a> {
    pub system: &'a str,
    /// 少样本示例（用户, 助手）：小模型照着示例的格式与粒度回答，比单纯的说明可靠得多。
    pub shots: &'a [(&'a str, &'a str)],
    pub user: &'a str,
    pub max_tokens: u32,
    /// GBNF 语法（llama.cpp）：约束输出为合法 JSON 等；为空则自由生成。
    pub grammar: Option<&'a str>,
}

/// 已加载进内存的本地大模型。
pub trait LanguageModel: Send + Sync {
    fn id(&self) -> &str;
    /// 生成完整回复。`cancel` 置位后尽快返回 `CoreError::Cancelled`。
    fn generate(&self, req: &LlmRequest<'_>, cancel: &AtomicBool) -> Result<String>;
}

/// 本地大模型的“仓库”：下载、校验、删除、加载。何时下载 / 加载 / 释放由核心层决定。
pub trait LlmProvider: Send + Sync {
    fn model_name(&self) -> &str;
    /// 下载完成后的文件大小。
    fn size_bytes(&self) -> u64;
    fn is_downloaded(&self) -> bool;
    /// 已下载的字节数（断点续传的起点）。
    fn partial_bytes(&self) -> u64;
    /// 下载并校验，支持断点续传。`progress(已下载, 总大小)`；`cancel` 置位后返回 `CoreError::Cancelled`。
    fn download(&self, progress: &mut dyn FnMut(u64, u64), cancel: &AtomicBool) -> Result<()>;
    /// 删除模型文件（含未完成的下载）。
    fn delete(&self) -> Result<()>;
    /// 加载进内存（数秒，在后台线程调用）。
    fn load(&self) -> Result<Arc<dyn LanguageModel>>;
}
