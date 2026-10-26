use serde::Serialize;

use crate::model::{
    ArticleId, CacheStats, PlayerState, SentenceAnalysis, SentenceSpan, SentenceTranslation,
    TranslatorStatus, WordInsight,
};

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
    /// 一句译文已写入缓存（后台预翻 / 智能精翻）。双语模式下 UI 据此补上这句译文。
    TranslationProgress {
        article_id: ArticleId,
        translation: SentenceTranslation,
        /// 已有译文的句数。
        translated: u32,
        total: u32,
    },
    /// 这篇的译文被整体作废（如术语表变化），UI 应重新读取 `translations`。
    TranslationsChanged {
        article_id: ArticleId,
    },
    /// 点词“举一反三”完成；失败时 `insight` 为空、`error` 有值，UI 退回词典释义。
    WordInsightReady {
        article_id: ArticleId,
        sentence: u32,
        word: u32,
        insight: Option<WordInsight>,
        error: Option<String>,
    },
    SentenceAnalysisReady {
        article_id: ArticleId,
        sentence: u32,
        analysis: Option<SentenceAnalysis>,
        error: Option<String>,
    },
    /// 翻译模式、模型下载进度（约 4 次/秒）或加载状态变化。
    TranslatorChanged {
        status: TranslatorStatus,
    },
    VocabularyChanged,
}

pub trait EventListener: Send + Sync {
    fn on_event(&self, event: CoreEvent);
}
