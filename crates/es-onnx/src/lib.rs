//! 全进程共用的 ONNX Runtime 环境与平台加速后端。
//!
//! Kokoro（TTS）和 Opus-MT（翻译）都跑在 ORT 上；ORT 每个进程只允许建一次环境，
//! 所以两个引擎必须共用这里的环境，而不能各自 `ort::init()`。

use std::sync::Mutex;

pub use ort;

/// ONNX Runtime 每个进程只允许建一次环境（ort 用只写一次的全局记录，
/// 即使旧环境已释放也不能再建），所以全进程共用一个，并且永不释放。
pub fn shared_env() -> ort::Result<ort::environment::Environment> {
    static ENV: Mutex<Option<ort::environment::Environment>> = Mutex::new(None);
    let mut slot = ENV.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(env) = slot.as_ref() {
        return Ok(env.clone());
    }
    let env = ort::init().with_name("english-speaker").build()?;
    *slot = Some(env.clone());
    Ok(env)
}

/// 平台加速后端：CoreML（Apple）/ NNAPI（Android）/ DirectML（Windows）；不可用时 ORT 自动回落 CPU。
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn platform_providers() -> Vec<ort::ep::ExecutionProviderDispatch> {
    vec![ort::ep::CoreML::default().build()]
}

#[cfg(target_os = "android")]
pub fn platform_providers() -> Vec<ort::ep::ExecutionProviderDispatch> {
    vec![ort::ep::NNAPI::default().build()]
}

#[cfg(target_os = "windows")]
pub fn platform_providers() -> Vec<ort::ep::ExecutionProviderDispatch> {
    vec![ort::ep::DirectML::default().build()]
}

/// Linux 等：没有平台加速后端，使用 CPU。
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "android",
    target_os = "windows"
)))]
pub fn platform_providers() -> Vec<ort::ep::ExecutionProviderDispatch> {
    Vec::new()
}
