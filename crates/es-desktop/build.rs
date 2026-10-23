//! Linux 上 CEF（libcef.so）放在可执行文件旁边：给主程序加 `$ORIGIN` rpath，安装包解压即可运行。
fn main() {
    let cef = std::env::var_os("CARGO_FEATURE_CEF").is_some();
    if cef && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo::rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
    }
}
