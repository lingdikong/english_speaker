//! 桌面 UI（Chromium 内的 Web 页面）与核心层之间的 JSON 协议。
//!
//! 请求：`{"cmd": "play_article", "id": "..."}`
//! 响应：`{"ok": true, "data": ...}` 或 `{"ok": false, "error": "..."}`
//! 事件：`CoreEvent` 的 JSON（`{"type": "player_position", ...}`），由宿主推送给页面。
//!
//! CEF 宿主中：页面通过 `cefQuery`/自定义 scheme 调用 `Bridge::handle_json`，
//! 事件经 `ExecuteJavaScript("window.__es_emit(...)")` 推回页面。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::audio::Volume;

use es_runtime::es_core::model::SourceKind;
use es_runtime::es_core::{AppCore, CoreEvent, EventListener, Result};

#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    ImportText {
        title: Option<String>,
        body: String,
        source_ref: Option<String>,
    },
    ListArticles,
    GetArticle {
        id: String,
    },
    DeleteArticle {
        id: String,
    },
    Timeline {
        id: String,
    },
    Voices,
    Synthesize {
        id: String,
        voice_id: Option<String>,
    },
    CancelSynthesis {
        id: String,
    },
    CacheStats,
    SetCacheLimit {
        bytes: u64,
    },
    SetPinned {
        id: String,
        pinned: bool,
    },
    RemoveDownload {
        id: String,
    },
    ClearCache,
    ListCollections,
    CreateCollection {
        name: String,
    },
    RenameCollection {
        id: String,
        name: String,
    },
    DeleteCollection {
        id: String,
    },
    AddToCollection {
        id: String,
        collection_id: String,
    },
    RemoveFromCollection {
        id: String,
        collection_id: String,
    },
    PlayArticle {
        id: String,
    },
    Play,
    Pause,
    Toggle,
    Seek {
        position_ms: u64,
    },
    Skip {
        delta_ms: i64,
    },
    SeekSentence {
        index: u32,
    },
    SetRate {
        rate: f32,
    },
    Stop,
    PlayerState,
    // ---- 宿主侧（不经核心层） ----
    /// 宿主信息：宿主类型、TTS 引擎、音量、是否支持变速。
    HostInfo,
    SetVolume {
        volume: f32,
    },
}

/// 宿主侧能力（音量等平台功能，不属于核心业务逻辑）。
#[derive(Default)]
pub struct HostInfo {
    /// `cef` / `dev` / `stdio`
    pub host: &'static str,
    /// `kokoro` / `mock`
    pub engine: &'static str,
    pub volume: Option<Arc<Volume>>,
}

pub struct Bridge {
    app: AppCore,
    host: HostInfo,
}

impl Bridge {
    pub fn new(app: AppCore) -> Self {
        Self {
            app,
            host: HostInfo::default(),
        }
    }

    pub fn with_host(mut self, host: HostInfo) -> Self {
        self.host = host;
        self
    }

    pub fn handle_json(&self, request: &str) -> String {
        let response = match serde_json::from_str::<Request>(request) {
            Ok(req) => match self.dispatch(req) {
                Ok(data) => json!({ "ok": true, "data": data }),
                Err(e) => json!({ "ok": false, "error": e.to_string() }),
            },
            Err(e) => json!({ "ok": false, "error": format!("bad request: {e}") }),
        };
        response.to_string()
    }

    fn dispatch(&self, req: Request) -> Result<Value> {
        let app = &self.app;
        let v = match req {
            Request::ImportText {
                title,
                body,
                source_ref,
            } => {
                let source = if source_ref.is_some() {
                    SourceKind::File
                } else {
                    SourceKind::Paste
                };
                to_value(app.import_text(title, body, source, source_ref)?)
            }
            Request::ListArticles => to_value(app.list_articles()?),
            Request::GetArticle { id } => to_value(app.get_article(&id)?),
            Request::DeleteArticle { id } => {
                app.delete_article(&id)?;
                Value::Null
            }
            Request::Timeline { id } => to_value(app.timeline(&id)?),
            Request::Voices => to_value(app.voices()),
            Request::Synthesize { id, voice_id } => {
                app.synthesize(&id, voice_id)?;
                Value::Null
            }
            Request::CancelSynthesis { id } => {
                app.cancel_synthesis(&id);
                Value::Null
            }
            Request::CacheStats => to_value(app.cache_stats()?),
            Request::SetCacheLimit { bytes } => {
                app.set_cache_limit(bytes)?;
                Value::Null
            }
            Request::SetPinned { id, pinned } => {
                app.set_pinned(&id, pinned)?;
                Value::Null
            }
            Request::RemoveDownload { id } => {
                app.remove_download(&id)?;
                Value::Null
            }
            Request::ClearCache => to_value(app.clear_cache()?),
            Request::ListCollections => to_value(app.list_collections()?),
            Request::CreateCollection { name } => to_value(app.create_collection(&name)?),
            Request::RenameCollection { id, name } => {
                app.rename_collection(&id, &name)?;
                Value::Null
            }
            Request::DeleteCollection { id } => {
                app.delete_collection(&id)?;
                Value::Null
            }
            Request::AddToCollection { id, collection_id } => {
                app.add_to_collection(&id, &collection_id)?;
                Value::Null
            }
            Request::RemoveFromCollection { id, collection_id } => {
                app.remove_from_collection(&id, &collection_id)?;
                Value::Null
            }
            Request::PlayArticle { id } => {
                app.play_article(&id)?;
                Value::Null
            }
            Request::Play => {
                app.play();
                Value::Null
            }
            Request::Pause => {
                app.pause();
                Value::Null
            }
            Request::Toggle => {
                app.toggle();
                Value::Null
            }
            Request::Seek { position_ms } => {
                app.seek(position_ms);
                Value::Null
            }
            Request::Skip { delta_ms } => {
                app.skip(delta_ms);
                Value::Null
            }
            Request::SeekSentence { index } => {
                app.seek_sentence(index)?;
                Value::Null
            }
            Request::SetRate { rate } => {
                app.set_rate(rate);
                Value::Null
            }
            Request::Stop => {
                app.stop();
                Value::Null
            }
            Request::PlayerState => to_value(app.player_state()),
            Request::HostInfo => json!({
                "host": self.host.host,
                // 无边框窗口按系统习惯摆放窗口按钮：macos 左上角红绿灯，其余右上角
                "os": std::env::consts::OS,
                "engine": self.host.engine,
                "volume": self.host.volume.as_ref().map_or(1.0, |v| v.get()),
                // 变速由 CpalSink 内置 WSOLA 时间伸缩实现（保持音调）
                "rate_supported": true,
            }),
            Request::SetVolume { volume } => {
                if let Some(v) = &self.host.volume {
                    v.set(volume);
                }
                Value::Null
            }
        };
        Ok(v)
    }
}

fn to_value<T: serde::Serialize>(v: T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// 把核心事件序列化为 JSON 交给宿主推送。
pub struct JsonEventListener<F: Fn(String) + Send + Sync>(pub F);

impl<F: Fn(String) + Send + Sync> EventListener for JsonEventListener<F> {
    fn on_event(&self, event: CoreEvent) {
        if let Ok(s) = serde_json::to_string(&event) {
            (self.0)(s);
        }
    }
}
