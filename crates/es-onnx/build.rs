//! Apple 平台：ONNX Runtime 的 CoreML EP 使用了 `@available`（依赖 `__isPlatformVersionAtLeast`），
//! 该符号在 clang 运行时库中；rustc 以 `-nodefaultlibs` 链接，需要显式链接。
//! 使用 `-bundle`：只交给最终链接器（可执行文件 / dylib），不打包进 staticlib；
//! Xcode 链接 App 时 clang 会自动带上运行时库。

use std::process::Command;

fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target = std::env::var("TARGET").unwrap_or_default();
    let lib = match os.as_str() {
        "macos" => "clang_rt.osx",
        "ios" if target.ends_with("-sim") || target.starts_with("x86_64") => "clang_rt.iossim",
        "ios" => "clang_rt.ios",
        _ => return,
    };
    let Ok(out) = Command::new("xcrun")
        .args(["clang", "-print-resource-dir"])
        .output()
    else {
        println!("cargo:warning=xcrun not found; cannot link {lib}");
        return;
    };
    let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
    println!("cargo:rustc-link-search=native={dir}/lib/darwin");
    println!("cargo:rustc-link-lib=static:-bundle={lib}");
}
