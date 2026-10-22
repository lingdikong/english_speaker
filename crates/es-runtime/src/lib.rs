//! 组装根（composition root）：把具体实现注入 `AppCore`。
//!
//! 移动端 FFI 与桌面端共用这一处装配逻辑，平台层只需提供
//! `AudioSink`（放声音）和 `EventListener`（更新 UI）。

use std::path::PathBuf;
use std::sync::Arc;

use es_audio::PcmS16Codec;
use es_core::ports::{AudioSink, TtsEngine};
use es_core::testing::MockTtsEngine;
use es_core::{AppConfig, AppCore, AppDeps, EventListener, Result};
use es_storage::SqliteStorage;
use es_tts_kokoro::KokoroEngine;

pub use es_core;

pub struct RuntimeConfig {
    /// 可写的应用数据目录。iOS: Application Support；Android: filesDir；桌面: 平台数据目录。
    pub data_dir: PathBuf,
    /// 只读的模型目录（随安装包分发）。iOS: Bundle；Android: install-time asset 解压目录。
    pub model_dir: Option<PathBuf>,
    pub default_voice: Option<String>,
    /// 缓存池默认上限；为空用核心默认值（1 GiB）。用户设置过则以数据库为准。
    pub cache_limit_bytes: Option<u64>,
}

impl RuntimeConfig {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            model_dir: None,
            default_voice: None,
            cache_limit_bytes: None,
        }
    }
}

pub fn build(
    config: RuntimeConfig,
    sink: Arc<dyn AudioSink>,
    listener: Arc<dyn EventListener>,
) -> Result<AppCore> {
    let tts = select_engine(config.model_dir.as_deref());
    build_with_engine(config, tts, sink, listener)
}

pub fn build_with_engine(
    config: RuntimeConfig,
    tts: Arc<dyn TtsEngine>,
    sink: Arc<dyn AudioSink>,
    listener: Arc<dyn EventListener>,
) -> Result<AppCore> {
    std::fs::create_dir_all(&config.data_dir)?;
    let storage = Arc::new(SqliteStorage::open(&config.data_dir.join("library.db"))?);
    log::info!("tts engine: {}", tts.id());

    let mut app_config = AppConfig::new(config.data_dir);
    app_config.default_voice = config.default_voice;
    if let Some(limit) = config.cache_limit_bytes {
        app_config.cache_limit_bytes = limit;
    }
    AppCore::new(
        app_config,
        AppDeps {
            storage,
            tts,
            codec: Arc::new(PcmS16Codec),
            sink,
            listener,
        },
    )
}

/// 有模型文件就用 Kokoro，否则退回 Mock 引擎（开发期）。
fn select_engine(model_dir: Option<&std::path::Path>) -> Arc<dyn TtsEngine> {
    if let Some(dir) = model_dir.filter(|d| KokoroEngine::is_available(d)) {
        match KokoroEngine::load(dir) {
            Ok(engine) => return Arc::new(engine),
            Err(e) => log::warn!("failed to load Kokoro, falling back to mock: {e}"),
        }
    }
    Arc::new(MockTtsEngine::default())
}

#[cfg(test)]
mod tests;
