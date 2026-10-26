//! 翻译与查词引擎，实现 `es-core` 的 `Translator` / `Dictionary` / `LlmProvider`。
//!
//! | 模块   | 实现                         | 分发             |
//! |--------|------------------------------|------------------|
//! | `mt`   | Opus-MT en→zh（ONNX，int8）  | 随包内置 ~108MB  |
//! | `dict` | ECDICT 核心词表（SQLite）    | 随包内置         |
//! | `llm`  | Qwen2.5-0.5B-Instruct（GGUF，llama.cpp）| 按需下载 ~430MB |

pub mod dict;
#[cfg(feature = "llm")]
pub mod llm;
pub mod mt;

pub use dict::EcDict;
#[cfg(feature = "llm")]
pub use llm::QwenProvider;
pub use mt::OpusMt;
