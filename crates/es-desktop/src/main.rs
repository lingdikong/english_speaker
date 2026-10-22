//! 桌面宿主（Windows / macOS / Linux）：Rust 核心 + Chromium（CEF）+ Web UI（ui/web）。
//!
//! ```text
//! es-desktop app   [--mute] [--devtools=9222]   桌面应用：CEF 窗口（需 `--features cef`）；
//!                                                未编译 CEF 时退回开发宿主（Chrome --app）
//! es-desktop serve [--mute] [--port=7450] [--open] [--devtools=9222]
//!                                                开发宿主：本地 HTTP + SSE 桥，可选用 Chrome --app 打开
//! es-desktop speak <file.txt> [--mute]           导入 → 合成 → 播放，终端里滚动显示当前句
//! es-desktop bridge [--mute]                     stdin 逐行读 JSON 请求，stdout 输出响应与事件（UI 联调）
//! ```
//!
//! 环境变量：`ES_DATA_DIR` 覆盖数据目录，`ES_MODEL_DIR` 指定模型目录，`ES_UI_DIR` 指定 UI 构建产物，
//! `ES_CHROME` 指定开发宿主使用的浏览器。

mod audio;
mod bridge;
#[cfg(feature = "cef")]
mod cef_host;
mod core_host;
mod dev_host;

use std::io::{BufRead, Read};
use std::sync::{mpsc, Arc};

use es_runtime::es_core::model::{PlayerStatus, SentenceSpan, SourceKind};
use es_runtime::es_core::text::connected::{self, Link};
use es_runtime::es_core::{CoreEvent, EventListener};

const USAGE: &str = "usage: es-desktop app [--mute] [--devtools=PORT]
       es-desktop serve [--mute] [--port=PORT] [--open] [--devtools=PORT]
       es-desktop speak <file.txt|-> [--mute]
       es-desktop bridge [--mute]";

fn main() {
    // Windows / Linux：CEF 子进程（渲染、GPU…）复用主程序
    #[cfg(all(feature = "cef", not(target_os = "macos")))]
    if std::env::args().any(|a| a.starts_with("--type=")) {
        let args = cef::args::Args::new();
        let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);
        std::process::exit(cef_host::renderer::run_subprocess(&args));
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    let value = |name: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(&format!("{name}=")).map(str::to_string))
    };
    let mute = flag("--mute");
    let devtools_port = value("--devtools").and_then(|v| v.parse().ok());
    let positional: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| !a.starts_with("--") && !a.starts_with("-psn"))
        .collect();
    let result = match positional.as_slice() {
        // macOS 从 Finder / `open` 启动 .app 时没有参数，等同于 `app`
        [] | ["app"] => run_app(mute, devtools_port),
        ["serve"] => dev_host::serve(dev_host::ServeOptions {
            port: value("--port").and_then(|v| v.parse().ok()).unwrap_or(7450),
            mute,
            open: flag("--open"),
            devtools_port,
            window_size: (1440, 900),
        }),
        ["speak", path] => speak(path, mute),
        ["bridge"] => run_bridge(mute),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

#[cfg(feature = "cef")]
fn run_app(mute: bool, devtools_port: Option<u16>) -> Result<(), Box<dyn std::error::Error>> {
    let code = cef_host::run(cef_host::HostOptions {
        mute,
        devtools_port,
        window_size: (1440, 900),
    })?;
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

#[cfg(not(feature = "cef"))]
fn run_app(mute: bool, devtools_port: Option<u16>) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("note: built without the `cef` feature; using the dev host (Chrome --app)");
    dev_host::serve(dev_host::ServeOptions {
        port: 0,
        mute,
        open: true,
        devtools_port,
        window_size: (1440, 900),
    })
}

struct ChannelListener(mpsc::Sender<CoreEvent>);

impl EventListener for ChannelListener {
    fn on_event(&self, event: CoreEvent) {
        let _ = self.0.send(event);
    }
}

fn speak(path: &str, mute: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut body = String::new();
    if path == "-" {
        std::io::stdin().read_to_string(&mut body)?;
    } else {
        body = std::fs::read_to_string(path)?;
    }
    let (tx, rx) = mpsc::channel();
    let app = es_runtime::build(
        core_host::runtime_config(),
        core_host::sink(mute, &Default::default()),
        Arc::new(ChannelListener(tx)),
    )?;
    let source_ref = (path != "-").then(|| path.to_string());
    let article = app.import_text(None, body, SourceKind::File, source_ref)?;
    println!(
        "imported \"{}\" ({} sentences)",
        article.title, article.total_sentences
    );
    // 导入即开始转换；立即播放，边听边转
    let t0 = std::time::Instant::now();
    let clock = || format!("{:>5.1}s", t0.elapsed().as_secs_f64());
    app.play_article(&article.id)?;

    let mut last_sentence = None;
    for event in rx {
        match event {
            CoreEvent::SynthesisProgress {
                article_id,
                ready,
                total,
                ..
            } if article_id == article.id => {
                println!("{}            convert {ready}/{total}", clock())
            }
            CoreEvent::SynthesisFailed {
                article_id,
                message,
            } if article_id == article.id => return Err(message.into()),
            CoreEvent::PlayerPosition {
                position_ms,
                sentence: Some(s),
                ..
            } if last_sentence != Some(s) => {
                last_sentence = Some(s);
                // 播放到某句时该句已合成：取最新时间轴以拿到连读标注
                if let Some(t) = app.timeline(&article.id)?.sentences.get(s as usize) {
                    let secs = position_ms as f64 / 1000.0;
                    println!("{} ▶ {secs:>6.1}s {}", clock(), connected::render_links(t));
                    let notes = describe_notes(t);
                    if !notes.is_empty() {
                        println!("                  {notes}");
                    }
                }
            }
            CoreEvent::PlayerStateChanged { state } => match state.status {
                PlayerStatus::Buffering => println!("{}            (buffering…)", clock()),
                PlayerStatus::Ended => break,
                _ => {}
            },
            _ => {}
        }
    }
    let stats = app.cache_stats()?;
    println!(
        "finished; cache {:.1} MB / {:.0} MB in {} entries",
        stats.used_bytes as f64 / 1e6,
        stats.limit_bytes as f64 / 1e6,
        stats.entries
    );
    Ok(())
}

/// 跟读提示：弱读、闪音、同化、省音。
fn describe_notes(s: &SentenceSpan) -> String {
    let clean = |t: &str| {
        t.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
            .to_string()
    };
    let mut parts = Vec::new();
    for (i, w) in s.words.iter().enumerate() {
        let word = clean(&w.text);
        let next = s.words.get(i + 1).map(|n| clean(&n.text));
        if let Some(weak) = &w.notes.weak_form {
            parts.push(format!("{word} /{weak}/"));
        }
        match (w.notes.link, next) {
            (Some(Link::Flap), Some(n)) => parts.push(format!("{word}‿{n} 闪音")),
            (Some(Link::Assimilation), Some(n)) => parts.push(format!("{word}‿{n} 同化")),
            (Some(Link::LinkingR), Some(n)) => parts.push(format!("{word}‿{n} 连接r")),
            (Some(Link::GlideJ), Some(n)) => parts.push(format!("{word}‿{n} 加 /j/")),
            (Some(Link::GlideW), Some(n)) => parts.push(format!("{word}‿{n} 加 /w/")),
            _ => {}
        }
        if w.notes.flap {
            parts.push(format!("{word} /{}/ 闪音", w.notes.ipa));
        }
        if w.notes.elided {
            parts.push(format!("{word} 词尾省音"));
        }
    }
    parts.join(" · ")
}

fn run_bridge(mute: bool) -> Result<(), Box<dyn std::error::Error>> {
    let listener = bridge::JsonEventListener(|json: String| println!("event {json}"));
    let bridge = core_host::start("stdio", mute, Arc::new(listener))?;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        println!("response {}", bridge.handle_json(&line));
    }
    Ok(())
}
