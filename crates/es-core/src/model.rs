//! 领域模型。全部可序列化，桌面端 JSON 桥与存储层直接复用。

use serde::{Deserialize, Serialize};

pub type ArticleId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Paste,
    File,
    Share,
}

/// 一篇文章的音频在缓存池中的状态（类似“下载状态”）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioStatus {
    /// 没有缓存（从未合成或已被淘汰）。播放时会重新流式合成。
    NotCached,
    /// 在合成队列中等待。
    Queued,
    /// 正在合成，已合成部分可以边听边转。
    Synthesizing,
    /// 合成了一部分后中断（被抢占、取消或 App 退出），下次从断点继续。
    Partial,
    /// 全部合成完毕，完整缓存在本地。
    Ready,
    Failed,
}

impl AudioStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotCached => "not_cached",
            Self::Queued => "queued",
            Self::Synthesizing => "synthesizing",
            Self::Partial => "partial",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "not_cached" => Self::NotCached,
            "queued" => Self::Queued,
            "synthesizing" => Self::Synthesizing,
            "partial" => Self::Partial,
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: ArticleId,
    pub title: String,
    pub source: SourceKind,
    /// 文件名 / 分享来源 URL 等，仅用于展示。
    pub source_ref: Option<String>,
    pub body: String,
    pub created_at_ms: i64,
    /// 用户为这篇文章选择的音色；为空时用默认音色。
    pub voice_id: Option<String>,
    /// 钉住（“保留下载”）：缓存池淘汰时跳过。
    pub pinned: bool,
    pub sentence_count: u32,
    /// 导入时按文本估算的总时长，合成完成前用于进度条。
    pub estimated_ms: u64,
}

/// 列表展示用，不含正文。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleSummary {
    pub id: ArticleId,
    pub title: String,
    pub source: SourceKind,
    pub source_ref: Option<String>,
    pub created_at_ms: i64,
    /// 缓存音频所用的音色；未缓存时为用户偏好（可能为空 = 默认音色）。
    pub voice_id: Option<String>,
    pub pinned: bool,
    pub status: AudioStatus,
    pub ready_sentences: u32,
    pub total_sentences: u32,
    pub cached_bytes: u64,
    /// `Ready` 时为真实时长，否则为估算值。
    pub duration_ms: u64,
    pub progress_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accent {
    American,
    British,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gender {
    Female,
    Male,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voice {
    pub id: String,
    pub name: String,
    pub accent: Accent,
    pub gender: Gender,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PcmFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

impl PcmFormat {
    pub fn frames_to_ms(&self, frames: u64) -> u64 {
        frames * 1000 / self.sample_rate as u64
    }
    pub fn ms_to_frames(&self, ms: u64) -> u64 {
        ms * self.sample_rate as u64 / 1000
    }
}

/// 一个词在原文中的位置及其在音频中的时间段（用于逐词高亮）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordSpan {
    /// 原文显示文本（未规范化）。
    pub text: String,
    /// 在所属句子 `text` 中的字节区间。
    pub char_start: u32,
    pub char_end: u32,
    pub start_ms: u64,
    pub end_ms: u64,
    /// 读音与连读标注（引擎不提供音素时为空）。
    #[serde(default)]
    pub notes: crate::text::connected::WordNote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentenceSpan {
    pub index: u32,
    pub paragraph: u32,
    pub text: String,
    /// 是否已合成。未合成的句子只有文本，时间均为 0（UI 置灰、不可点击）。
    pub ready: bool,
    pub start_ms: u64,
    pub end_ms: u64,
    pub words: Vec<WordSpan>,
}

/// 音频与原文的对齐时间轴，相当于 Apple Music 的“逐字歌词”。
/// 导入时就有全部句子文本；合成过程中逐句补上时间。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Timeline {
    pub sentences: Vec<SentenceSpan>,
    pub complete: bool,
}

impl Timeline {
    /// 返回 `ms` 时刻所在的（句子下标, 词下标）。落在句间停顿时返回前一句、无词。
    pub fn locate(&self, ms: u64) -> (Option<u32>, Option<u32>) {
        let ready = self.sentences.partition_point(|s| s.ready);
        let idx = self.sentences[..ready].partition_point(|s| s.start_ms <= ms);
        if idx == 0 {
            return (None, None);
        }
        let s = &self.sentences[idx - 1];
        let word = s
            .words
            .iter()
            .position(|w| w.start_ms <= ms && ms < w.end_ms)
            .map(|w| w as u32);
        (Some(s.index), word)
    }

    pub fn sentence_start_ms(&self, index: u32) -> Option<u64> {
        self.sentences
            .get(index as usize)
            .filter(|s| s.ready)
            .map(|s| s.start_ms)
    }
}

/// 一句话对应的音频块在缓存文件中的位置。块内含句前停顿 + 语音。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    pub byte_offset: u64,
    pub byte_len: u64,
    /// 块在整篇音频中的起始帧。
    pub start_frame: u64,
    pub frames: u64,
}

impl Chunk {
    pub fn end_frame(&self) -> u64 {
        self.start_frame + self.frames
    }
}

/// 存储层中的一句：文本 + 时间轴 + （若已合成）音频块位置。
#[derive(Debug, Clone)]
pub struct SentenceRecord {
    pub span: SentenceSpan,
    pub chunk: Option<Chunk>,
}

/// 缓存池中的一条记录：一篇文章的音频缓存。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub article_id: ArticleId,
    pub voice_id: String,
    pub codec: String,
    /// 相对于音频目录的文件名。不存绝对路径：iOS 沙盒路径会随 App 更新变化。
    pub file_name: String,
    pub sample_rate: u32,
    pub state: AudioStatus,
    pub error: Option<String>,
    pub ready_sentences: u32,
    pub total_sentences: u32,
    pub ready_frames: u64,
    pub byte_size: u64,
    pub created_at_ms: i64,
    pub last_access_ms: i64,
}

impl CacheEntry {
    pub fn format(&self) -> PcmFormat {
        PcmFormat {
            sample_rate: self.sample_rate,
            channels: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub used_bytes: u64,
    pub limit_bytes: u64,
    pub pinned_bytes: u64,
    pub entries: u32,
}

/// 合成完成前用“已合成的真实时长 + 未合成部分的估算”作为总时长。
pub fn blended_duration_ms(
    estimated_ms: u64,
    ready_ms: u64,
    ready: u32,
    total: u32,
    complete: bool,
) -> u64 {
    if complete || total == 0 {
        return ready_ms;
    }
    let remaining = estimated_ms * (total.saturating_sub(ready)) as u64 / total as u64;
    ready_ms + remaining
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerStatus {
    Idle,
    Playing,
    /// 播放追上了合成进度，等待下一句合成完成（类似网络缓冲）。
    Buffering,
    Paused,
    Ended,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub article_id: Option<ArticleId>,
    pub status: PlayerStatus,
    pub position_ms: u64,
    /// 已合成可播放到的位置（进度条上的“已缓冲”段）。
    pub buffered_ms: u64,
    pub duration_ms: u64,
    /// `duration_ms` 是否为最终值（合成完成前为估算）。
    pub duration_final: bool,
    pub rate: f32,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            article_id: None,
            status: PlayerStatus::Idle,
            position_ms: 0,
            buffered_ms: 0,
            duration_ms: 0,
            duration_final: false,
            rate: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sentence(index: u32, start: u64, end: u64, ready: bool) -> SentenceSpan {
        SentenceSpan {
            index,
            paragraph: 0,
            text: String::new(),
            ready,
            start_ms: start,
            end_ms: end,
            words: vec![WordSpan {
                text: "w".into(),
                char_start: 0,
                char_end: 1,
                start_ms: start,
                end_ms: end,
                notes: Default::default(),
            }],
        }
    }

    #[test]
    fn locate_finds_sentence_and_word() {
        let t = Timeline {
            sentences: vec![
                sentence(0, 0, 1000, true),
                sentence(1, 1200, 2000, true),
                sentence(2, 0, 0, false),
            ],
            complete: false,
        };
        assert_eq!(t.locate(500), (Some(0), Some(0)));
        assert_eq!(t.locate(1100), (Some(0), None));
        assert_eq!(t.locate(1500), (Some(1), Some(0)));
        assert_eq!(t.locate(9999), (Some(1), None));
        assert_eq!(t.sentence_start_ms(2), None);
    }
}
