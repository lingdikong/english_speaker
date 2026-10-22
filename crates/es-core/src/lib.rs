//! English Speaker 核心层。
//!
//! 只包含与平台无关的逻辑：领域模型、文本处理、合成编排、播放引擎。
//! 所有与外部世界的交互（TTS 推理、存储、编解码、声卡输出、事件回调）
//! 都通过 [`ports`] 中的 trait 注入，由上层（`es-runtime` / 平台壳）提供实现。

pub mod app;
mod cache;
pub mod error;
pub mod events;
pub mod model;
pub mod ports;
pub mod testing;
pub mod text;

mod player;
mod synth;

pub use app::{AppConfig, AppCore, AppDeps};
pub use cache::DEFAULT_LIMIT_BYTES as DEFAULT_CACHE_LIMIT_BYTES;
pub use error::{CoreError, Result};
pub use events::{CoreEvent, EventListener};
