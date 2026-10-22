//! 桌面宿主共用的核心装配：数据/模型/UI 目录、音频输出、首次启动的示例文章。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use es_runtime::es_core::model::SourceKind;
use es_runtime::es_core::ports::AudioSink;
use es_runtime::es_core::testing::NullSink;
use es_runtime::es_core::{AppCore, EventListener};
use es_runtime::RuntimeConfig;

use crate::audio::{CpalSink, Volume};
use crate::bridge::{Bridge, HostInfo};

/// 首次启动（空资料库）预置的示例文章：（文件名, 内容）。第一行为标题。
const SAMPLES: &[(&str, &str)] = &[
    (
        "01-morning-coffee.txt",
        include_str!("../samples/01-morning-coffee.txt"),
    ),
    (
        "02-learning-by-listening.txt",
        include_str!("../samples/02-learning-by-listening.txt"),
    ),
    (
        "03-a-walk-in-the-rain.txt",
        include_str!("../samples/03-a-walk-in-the-rain.txt"),
    ),
];

pub fn runtime_config() -> RuntimeConfig {
    let mut config = RuntimeConfig::new(data_dir());
    config.model_dir = std::env::var_os("ES_MODEL_DIR")
        .map(PathBuf::from)
        .or_else(bundled_model_dir);
    config
}

pub fn data_dir() -> PathBuf {
    std::env::var_os("ES_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(default_data_dir)
}

/// 各平台约定的用户数据目录。
fn default_data_dir() -> PathBuf {
    let home = || {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
    };
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("EnglishSpeaker")
    } else if cfg!(target_os = "macos") {
        home().join("Library/Application Support/EnglishSpeaker")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".local/share"))
            .join("english-speaker")
    }
}

/// 安装包内的资源目录：macOS `.app/Contents/Resources/<name>`，其他平台为可执行文件旁的 `<name>/`。
fn bundled_dir(name: &str) -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    [exe_dir.join("../Resources").join(name), exe_dir.join(name)]
        .into_iter()
        .find(|p| p.is_dir())
}

fn bundled_model_dir() -> Option<PathBuf> {
    bundled_dir("models")
}

/// Web UI 构建产物目录：`ES_UI_DIR` > 安装包内 `ui/` > 源码树 `ui/web/dist`（开发）。
pub fn ui_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("ES_UI_DIR") {
        return Some(PathBuf::from(dir));
    }
    bundled_dir("ui").or_else(|| {
        let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/web/dist");
        dev.join("index.html").is_file().then_some(dev)
    })
}

pub fn sink(mute: bool, volume: &Arc<Volume>) -> Arc<dyn AudioSink> {
    if mute {
        Arc::new(NullSink::default())
    } else {
        Arc::new(CpalSink::with_volume(volume.clone()))
    }
}

/// 启动核心并包成 JSON 桥。事件经 `listener` 推给页面。
pub fn start(
    host: &'static str,
    mute: bool,
    listener: Arc<dyn EventListener>,
) -> Result<Bridge, Box<dyn std::error::Error>> {
    let config = runtime_config();
    let engine = if config
        .model_dir
        .as_deref()
        .is_some_and(es_tts_kokoro::KokoroEngine::is_available)
    {
        "kokoro"
    } else {
        "mock"
    };
    let data_dir = config.data_dir.clone();
    let volume = Arc::new(Volume::default());
    let app = es_runtime::build(config, sink(mute, &volume), listener)?;
    // 桌面应用首次启动预置示例；stdio 联调模式不动资料库
    if host != "stdio" {
        seed_samples(&app, &data_dir);
    }
    Ok(Bridge::new(app).with_host(HostInfo {
        host,
        engine,
        volume: Some(volume),
    }))
}

/// 首次启动且资料库为空时导入示例文章（只做一次，用户删光后不再预置）。
fn seed_samples(app: &AppCore, data_dir: &Path) {
    let marker = data_dir.join(".samples-seeded");
    if marker.exists() {
        return;
    }
    if app.list_articles().map(|l| l.is_empty()).unwrap_or(false) {
        for (name, text) in SAMPLES {
            let (title, body) = text.split_once('\n').unwrap_or(("", text));
            if let Err(e) = app.import_text(
                Some(title.trim().to_string()),
                body.trim().to_string(),
                SourceKind::File,
                Some("Sample".to_string()),
            ) {
                log::warn!("seed sample {name} failed: {e}");
            }
        }
    }
    let _ = std::fs::write(marker, b"");
}
