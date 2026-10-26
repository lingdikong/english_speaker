//! 组装根（composition root）：把具体实现注入 `AppCore`。
//!
//! 移动端 FFI 与桌面端共用这一处装配逻辑，平台层只需提供
//! `AudioSink`（放声音）和 `EventListener`（更新 UI）。

use std::path::PathBuf;
use std::sync::Arc;

use es_audio::PcmS16Codec;
use es_core::ports::{AudioSink, TtsEngine};
use es_core::testing::MockTtsEngine;
use es_core::{AppConfig, AppCore, AppDeps, EventListener, Result, TranslationDeps};
use es_storage::SqliteStorage;
use es_translate::{EcDict, OpusMt};
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
    /// 智能翻译模型的下载地址（自建镜像），按顺序尝试；为空用内置地址（ModelScope → HuggingFace）。
    pub llm_urls: Vec<String>,
}

impl RuntimeConfig {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            model_dir: None,
            default_voice: None,
            cache_limit_bytes: None,
            llm_urls: Vec::new(),
        }
    }
}

pub fn build(
    config: RuntimeConfig,
    sink: Arc<dyn AudioSink>,
    listener: Arc<dyn EventListener>,
) -> Result<AppCore> {
    let tts = select_engine(config.model_dir.as_deref());
    let translation = select_translation(&config);
    build_with_engines(config, tts, translation, sink, listener)
}

pub fn build_with_engine(
    config: RuntimeConfig,
    tts: Arc<dyn TtsEngine>,
    sink: Arc<dyn AudioSink>,
    listener: Arc<dyn EventListener>,
) -> Result<AppCore> {
    build_with_engines(config, tts, TranslationDeps::default(), sink, listener)
}

pub fn build_with_engines(
    config: RuntimeConfig,
    tts: Arc<dyn TtsEngine>,
    translation: TranslationDeps,
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
            translation,
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

/// 翻译与查词：模型目录里有什么就启用什么；智能翻译模型下载到可写的 `<data_dir>/models`。
fn select_translation(config: &RuntimeConfig) -> TranslationDeps {
    let mut deps = TranslationDeps::default();
    if let Some(dir) = config.model_dir.as_deref() {
        if OpusMt::is_available(dir) {
            // 与 TTS 同时跑：给 MT 两个线程，避免抢占合成
            match OpusMt::load(dir, 2) {
                Ok(mt) => deps.translator = Some(Arc::new(mt)),
                Err(e) => log::warn!("failed to load Opus-MT: {e}"),
            }
        }
        if EcDict::is_available(dir) {
            match EcDict::open(dir) {
                Ok(d) => deps.dictionary = Some(Arc::new(d)),
                Err(e) => log::warn!("failed to open dictionary: {e}"),
            }
        }
    }
    #[cfg(feature = "llm")]
    {
        let provider = es_translate::QwenProvider::new(config.data_dir.join("models"))
            .with_urls(config.llm_urls.clone());
        deps.llm = Some(Arc::new(provider));
    }
    log::info!(
        "translation: mt={} dict={} llm={}",
        deps.translator.is_some(),
        deps.dictionary.is_some(),
        deps.llm.is_some()
    );
    deps
}

#[cfg(test)]
mod tests;
