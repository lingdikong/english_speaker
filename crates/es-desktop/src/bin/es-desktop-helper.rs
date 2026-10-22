//! macOS CEF helper（渲染 / GPU / 网络等子进程）。由 bundle 脚本复制为
//! `es-desktop Helper*.app/Contents/MacOS/es-desktop Helper*`。

#[cfg(target_os = "macos")]
#[path = "../cef_host/renderer.rs"]
mod renderer;

#[cfg(target_os = "macos")]
fn main() {
    let args = cef::args::Args::new();
    // helper 位于 Frameworks/xxx Helper.app/Contents/MacOS/，按 helper 布局加载 CEF framework
    let loader = cef::library_loader::LibraryLoader::new(
        &std::env::current_exe().expect("current_exe"),
        true,
    );
    assert!(loader.load(), "failed to load CEF framework");
    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);
    std::process::exit(renderer::run_subprocess(&args));
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("es-desktop-helper is only used on macOS; Windows/Linux reuse the main executable");
    std::process::exit(2);
}
