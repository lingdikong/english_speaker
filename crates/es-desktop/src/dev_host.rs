//! 开发用宿主：本地 HTTP 服务 + Server-Sent Events，用已安装的 Chrome 以 `--app=` 方式打开。
//!
//! 与 CEF 宿主共用同一份前端与 JSON 桥（前端 `bridge.ts` 自动识别宿主）：
//!
//! ```text
//! GET  /…               ui/web/dist 下的静态文件
//! POST /__es/invoke     请求体为 JSON 请求，响应为 `Bridge::handle_json` 的结果
//! GET  /__es/events     text/event-stream，每条核心事件一条 `data:`
//! ```
//!
//! 只监听 127.0.0.1，纯标准库实现（线程 / 连接），无额外依赖。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::bridge::{Bridge, JsonEventListener};
use crate::core_host;

pub struct ServeOptions {
    pub port: u16,
    pub mute: bool,
    /// 启动后用 Chrome `--app` 打开窗口；窗口关闭即退出。
    pub open: bool,
    /// Chrome 的 `--remote-debugging-port`（CDP 截图用）。
    pub devtools_port: Option<u16>,
    pub window_size: (u32, u32),
}

type Subscribers = Arc<Mutex<Vec<mpsc::Sender<Arc<str>>>>>;

pub fn serve(opts: ServeOptions) -> Result<(), Box<dyn std::error::Error>> {
    let ui = core_host::ui_dir().ok_or(
        "找不到 Web UI 构建产物：先在 ui/web 下运行 `npm install && npm run build`，或设置 ES_UI_DIR",
    )?;
    let subs: Subscribers = Arc::default();
    let listener = {
        let subs = subs.clone();
        JsonEventListener(move |json: String| {
            let msg: Arc<str> = json.into();
            subs.lock()
                .unwrap()
                .retain(|tx| tx.send(msg.clone()).is_ok());
        })
    };
    let bridge = Arc::new(core_host::start("dev", opts.mute, Arc::new(listener))?);

    let server = TcpListener::bind(("127.0.0.1", opts.port))?;
    let addr = server.local_addr()?;
    let url = format!("http://{addr}/");
    println!("es-desktop dev host: {url}  (ui: {})", ui.display());

    let ui = Arc::new(ui);
    thread::Builder::new()
        .name("es-http".into())
        .spawn(move || {
            for stream in server.incoming().flatten() {
                let (bridge, subs, ui) = (bridge.clone(), subs.clone(), ui.clone());
                let _ = thread::Builder::new()
                    .name("es-http-conn".into())
                    .spawn(move || {
                        if let Err(e) = handle(stream, &bridge, &subs, &ui) {
                            log::debug!("http connection: {e}");
                        }
                    });
            }
        })?;

    if opts.open {
        let mut chrome = launch_chrome(&url, &opts)?;
        let status = chrome.wait()?;
        println!("browser exited ({status}); shutting down");
    } else {
        // 只提供服务（例如配合 `npm run dev` 或 headless 截图）
        loop {
            thread::park();
        }
    }
    Ok(())
}

struct Req {
    method: String,
    path: String,
    body: Vec<u8>,
}

fn read_request(stream: &TcpStream) -> std::io::Result<Option<Req>> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let mut len = 0usize;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                len = v.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0; len.min(16 << 20)];
    reader.read_exact(&mut body)?;
    Ok(Some(Req { method, path, body }))
}

fn respond(mut s: &TcpStream, status: &str, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    s.write_all(body)?;
    s.flush()
}

fn handle(
    stream: TcpStream,
    bridge: &Bridge,
    subs: &Subscribers,
    ui: &Path,
) -> std::io::Result<()> {
    let Some(req) = read_request(&stream)? else {
        return Ok(());
    };
    let path = req.path.split('?').next().unwrap_or("/");
    match (req.method.as_str(), path) {
        ("POST", "/__es/invoke") => {
            let body = String::from_utf8_lossy(&req.body);
            let resp = bridge.handle_json(&body);
            respond(&stream, "200 OK", "application/json", resp.as_bytes())
        }
        ("GET", "/__es/events") => event_stream(stream, subs),
        ("GET", _) => match static_file(ui, path) {
            Some((bytes, ctype)) => respond(&stream, "200 OK", ctype, &bytes),
            None => respond(&stream, "404 Not Found", "text/plain", b"not found"),
        },
        _ => respond(&stream, "405 Method Not Allowed", "text/plain", b""),
    }
}

fn event_stream(mut stream: TcpStream, subs: &Subscribers) -> std::io::Result<()> {
    stream.write_all(
        b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: keep-alive\r\n\r\n: connected\n\n",
    )?;
    let (tx, rx) = mpsc::channel::<Arc<str>>();
    subs.lock().unwrap().push(tx);
    loop {
        match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(msg) => write!(stream, "data: {msg}\n\n")?,
            // 心跳：发现断开的连接
            Err(RecvTimeoutError::Timeout) => stream.write_all(b": ping\n\n")?,
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
        stream.flush()?;
    }
}

/// 把 URL 路径映射为 UI 目录下的文件（拒绝 `..`），空路径回落到 index.html。
pub fn resolve(root: &Path, url_path: &str) -> Option<PathBuf> {
    let rel = url_path.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    let rel = PathBuf::from(percent_decode(rel));
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    let path = root.join(rel);
    path.is_file().then_some(path)
}

fn static_file(root: &Path, url_path: &str) -> Option<(Vec<u8>, &'static str)> {
    let path = resolve(root, url_path)?;
    let bytes = std::fs::read(&path).ok()?;
    Some((bytes, mime(&path)))
}

pub fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 已安装的 Chrome / Chromium / Edge。
fn find_chrome() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ES_CHROME") {
        return Some(PathBuf::from(p));
    }
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &[
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ]
    } else if cfg!(target_os = "windows") {
        &[
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        ]
    } else {
        &[
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/snap/bin/chromium",
        ]
    };
    candidates.iter().map(PathBuf::from).find(|p| p.is_file())
}

fn launch_chrome(url: &str, opts: &ServeOptions) -> Result<Child, Box<dyn std::error::Error>> {
    let chrome = find_chrome().ok_or("找不到 Chrome / Chromium，可用 ES_CHROME 指定路径")?;
    // 独立的用户目录：保证启动新实例（窗口关闭 = 进程退出），不影响用户自己的浏览器
    let profile = core_host::data_dir().join("chrome-profile");
    let mut cmd = Command::new(chrome);
    cmd.arg(format!("--app={url}"))
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!(
            "--window-size={},{}",
            opts.window_size.0, opts.window_size.1
        ))
        .args([
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-features=Translate",
            "--autoplay-policy=no-user-gesture-required",
        ]);
    if let Some(port) = opts.devtools_port {
        cmd.arg(format!("--remote-debugging-port={port}"));
    }
    Ok(cmd.spawn()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_file_rejects_traversal() {
        let root = std::env::temp_dir();
        assert!(static_file(&root, "/../etc/passwd").is_none());
        assert!(static_file(&root, "/%2e%2e/etc/passwd").is_none());
    }

    #[test]
    fn decodes_percent() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
