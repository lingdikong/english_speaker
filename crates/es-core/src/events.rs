use serde::Serialize;

use crate::model::{ArticleId, CacheStats, PlayerState, SentenceSpan};

/// 核心层向 UI 推送的事件。
///
/// 事件可能来自后台线程（合成线程、播放线程），平台层需要自行切回主线程。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoreEvent {
    /// 列表需要刷新（导入、删除、状态变化、淘汰）。
    LibraryChanged,
    /// 一句合成完毕并已写入缓存，可以播放。UI 据此把原文中该句点亮。
    SynthesisProgress {
        article_id: ArticleId,
        ready: u32,
        total: u32,
        sentence: SentenceSpan,
    },
    SynthesisFinished {
        article_id: ArticleId,
        duration_ms: u64,
    },
    SynthesisFailed {
        article_id: ArticleId,
        message: String,
    },
    CacheChanged {
        stats: CacheStats,
    },
    PlayerStateChanged {
        state: PlayerState,
    },
    /// 播放中约每 100ms 推送一次。
    PlayerPosition {
        position_ms: u64,
        buffered_ms: u64,
        sentence: Option<u32>,
        word: Option<u32>,
    },
}

pub trait EventListener: Send + Sync {
    fn on_event(&self, event: CoreEvent);
}
