//! 核心层对外依赖的抽象（端口）。
//!
//! | trait          | 由谁实现                                   |
//! |----------------|--------------------------------------------|
//! | `TtsEngine`    | `es-tts-kokoro`（ONNX 推理）               |
//! | `Storage`      | `es-storage`（SQLite）                     |
//! | `AudioCodec`   | `es-audio`（PCM s16，后续 Opus）           |
//! | `AudioSink`    | 平台层：iOS AVAudioEngine / Android AudioTrack / 桌面 cpal |
//! | `EventListener`| 平台层：UI 状态更新                        |

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
