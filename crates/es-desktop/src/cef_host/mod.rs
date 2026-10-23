//! CEF（Chromium Embedded Framework）窗口宿主。
//!
//! - 页面：`https://es.invalid/…` 由 `ResourceRequestHandler` 从 Web UI 构建产物目录直接返回
//!   （`.invalid` 是保留顶级域，请求永远不会出网）。
//! - 请求：页面调用 `window.cefQuery({request})`（CEF message router，渲染进程 ↔ 浏览器进程 IPC），
//!   浏览器进程在专用工作线程上按序调用 `Bridge::handle_json`，结果经回调返回页面。
//! - 事件：核心事件序列化为 JSON 后 `ExecuteJavaScript("window.__es_emit(...)")` 推回页面。
//! - 音频：沿用 `CpalSink`（Rust 侧直接出声，不经浏览器）。
//!
//! 进程模型：macOS 子进程走 `es-desktop-helper`（见 `renderer.rs`）；Windows / Linux 复用主程序，
//! 由 `main` 按 `--type=` 分流到 [`renderer::run_subprocess`]。

pub mod renderer;

#[cfg(target_os = "macos")]
mod mac;

use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;

use cef::wrapper::message_router::{
    BrowserSideCallback, BrowserSideHandler, BrowserSideRouter, MessageRouterBrowserSide,
    MessageRouterBrowserSideHandlerCallbacks, MessageRouterConfig,
};
use cef::wrapper::stream_resource_handler::StreamResourceHandler;
use cef::*;

use crate::bridge::JsonEventListener;
use crate::core_host;

/// 页面来源。
const ORIGIN: &str = "https://es.invalid/";

pub struct HostOptions {
    pub mute: bool,
    pub devtools_port: Option<u16>,
    pub window_size: (i32, i32),
}

type Query = (String, Arc<Mutex<dyn BrowserSideCallback>>);

/// 浏览器进程内共享的状态。
pub struct Shared {
    opts: HostOptions,
    ui_dir: PathBuf,
    router: Arc<BrowserSideRouter>,
    /// 主窗口的浏览器（事件推送目标）。CEF 允许在浏览器进程任意线程调用 `CefFrame` 方法。
    browser: Mutex<Option<Browser>>,
    /// 主窗口（无边框窗口控制按钮用）。只在 UI 线程访问其内容。
    window: Mutex<Option<Window>>,
    /// 桥接请求按序交给工作线程处理，不阻塞 UI 线程。
    queries: Mutex<Option<mpsc::Sender<Query>>>,
}

static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

impl Shared {
    fn emit(&self, json: &str) {
        let browser = self.browser.lock().unwrap().clone();
        if let Some(frame) = browser.and_then(|b| b.main_frame()) {
            let code = format!("window.__es_emit&&window.__es_emit({json})");
            frame.execute_java_script(Some(&CefString::from(code.as_str())), None, 0);
        }
    }

    /// 关闭所有浏览器（macOS ⌘Q / 菜单“退出”）。
    pub fn close_all(&self) {
        if let Some(host) = self.browser.lock().unwrap().as_ref().and_then(|b| b.host()) {
            host.close_browser(0);
        }
    }
}

pub fn shared() -> Option<&'static Arc<Shared>> {
    SHARED.get()
}

/// 浏览器进程入口：初始化 CEF、创建窗口、运行消息循环，返回退出码。
pub fn run(opts: HostOptions) -> Result<i32, Box<dyn std::error::Error>> {
    let ui_dir = core_host::ui_dir().ok_or(
        "找不到 Web UI 构建产物：先在 ui/web 下运行 `npm install && npm run build`，或设置 ES_UI_DIR",
    )?;

    #[cfg(target_os = "macos")]
    let _loader = {
        let loader = library_loader::LibraryLoader::new(&std::env::current_exe()?, false);
        if !loader.load() {
            return Err("无法加载 Chromium Embedded Framework（需在 .app bundle 中运行，见 scripts/bundle-macos.sh）".into());
        }
        loader
    };
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

    let args = cef::args::Args::new();

    #[cfg(target_os = "macos")]
    mac::setup_application();

    let shared = Arc::new(Shared {
        opts,
        ui_dir,
        router: BrowserSideRouter::new(MessageRouterConfig::default()),
        browser: Mutex::new(None),
        window: Mutex::new(None),
        queries: Mutex::new(None),
    });
    let _ = SHARED.set(shared.clone());

    let mut app = BrowserApp::new(shared.clone());
    // 主程序也可能被当作子进程启动（Windows / Linux 未单独打包 helper 时），此时直接返回
    let code = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    if code >= 0 {
        return Ok(code);
    }

    let cache = core_host::data_dir().join("cef");
    let settings = Settings {
        no_sandbox: 1,
        root_cache_path: CefString::from(cache.to_string_lossy().as_ref()),
        cache_path: CefString::from(cache.join("default").to_string_lossy().as_ref()),
        remote_debugging_port: shared.opts.devtools_port.map_or(0, i32::from),
        log_severity: LogSeverity::WARNING,
        // 深色背景，避免首帧白闪
        background_color: 0xff1e1e1e,
        ..Default::default()
    };
    if initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    ) != 1
    {
        return Err("CEF initialize failed".into());
    }

    #[cfg(target_os = "macos")]
    let _delegate = mac::setup_delegate();

    run_message_loop();
    shutdown();
    Ok(0)
}

/// 在 CEF 上下文就绪后（UI 线程）：启动核心、注册桥接、创建窗口。
fn on_context_initialized(shared: &Arc<Shared>) {
    let events = shared.clone();
    let listener = JsonEventListener(move |json: String| events.emit(&json));
    let bridge = match core_host::start("cef", shared.opts.mute, Arc::new(listener)) {
        Ok(b) => Arc::new(b),
        Err(e) => {
            eprintln!("error: failed to start core: {e}");
            quit_message_loop();
            return;
        }
    };

    // 桥接工作线程：按到达顺序处理请求（播放 → 跳句 这类序列不会乱序）
    let (tx, rx) = mpsc::channel::<Query>();
    thread::Builder::new()
        .name("es-bridge".into())
        .spawn(move || {
            for (request, callback) in rx {
                let response = bridge.handle_json(&request);
                callback.lock().unwrap().success_str(&response);
            }
        })
        .expect("spawn bridge thread");
    *shared.queries.lock().unwrap() = Some(tx);
    shared
        .router
        .add_handler(Arc::new(BridgeQueryHandler(shared.clone())), true);

    let mut client = EsClient::new(shared.clone());
    let url = CefString::from(format!("{ORIGIN}index.html").as_str());
    let settings = BrowserSettings {
        background_color: 0xff1e1e1e,
        ..Default::default()
    };
    let mut bv_delegate = EsBrowserViewDelegate::new();
    let browser_view = browser_view_create(
        Some(&mut client),
        Some(&url),
        Some(&settings),
        None,
        None,
        Some(&mut bv_delegate),
    );
    let mut window_delegate = EsWindowDelegate::new(
        std::cell::RefCell::new(browser_view),
        shared.clone(),
        shared.opts.window_size,
    );
    window_create_top_level(Some(&mut window_delegate));
}

struct BridgeQueryHandler(Arc<Shared>);

impl BrowserSideHandler for BridgeQueryHandler {
    fn on_query_str(
        &self,
        _browser: Option<Browser>,
        _frame: Option<Frame>,
        _query_id: i64,
        request: &str,
        _persistent: bool,
        callback: Arc<Mutex<dyn BrowserSideCallback>>,
    ) -> bool {
        // 无边框窗口的控制命令：不经核心桥，投递到 UI 线程直接操作 CEF 窗口
        let cmd = serde_json::from_str::<serde_json::Value>(request)
            .ok()
            .and_then(|v| v.get("cmd").and_then(|c| c.as_str()).map(str::to_string))
            .unwrap_or_default();
        if cmd.starts_with("window_") {
            let mut task = WindowCommandTask::new(cmd);
            post_task(ThreadId::UI, Some(&mut task));
            callback.lock().unwrap().success_str(r#"{"ok":true,"data":null}"#);
            return true;
        }
        match self.0.queries.lock().unwrap().as_ref() {
            Some(tx) => tx.send((request.to_string(), callback)).is_ok(),
            None => false,
        }
    }
}

// 在 UI 线程执行窗口控制（最小化 / 最大化还原 / 关闭）。
wrap_task! {
    struct WindowCommandTask {
        cmd: String,
    }

    impl Task {
        fn execute(&self) {
            let Some(shared) = SHARED.get() else { return };
            let Some(window) = shared.window.lock().unwrap().clone() else { return };
            match self.cmd.as_str() {
                "window_minimize" => window.minimize(),
                "window_maximize" => {
                    if window.is_maximized() != 0 {
                        window.restore();
                    } else {
                        window.maximize();
                    }
                }
                // 与 ⌘Q 同一路径：有序关闭浏览器 → on_before_close → 退出消息循环
                "window_close" => shared.close_all(),
                _ => {}
            }
        }
    }
}

/// 把 `https://es.invalid/<path>` 映射到 UI 目录下的文件。
fn resource_for(ui_dir: &std::path::Path, url: &str) -> Option<ResourceHandler> {
    let path = url.strip_prefix(ORIGIN)?;
    let path = path.split(['?', '#']).next().unwrap_or("");
    let file = crate::dev_host::resolve(ui_dir, path)?;
    let mime = crate::dev_host::mime(&file);
    let stream =
        stream_reader_create_for_file(Some(&CefString::from(file.to_string_lossy().as_ref())))?;
    Some(StreamResourceHandler::new_with_stream(
        mime.split(';').next().unwrap_or(mime).to_string(),
        stream,
    ))
}

// ---------------------------------------------------------------- App

wrap_app! {
    pub struct BrowserApp {
        shared: Arc<Shared>,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            // 只处理浏览器进程（process_type 为空）
            if process_type.is_some_and(|t| !t.to_string().is_empty()) {
                return;
            }
            if let Some(cl) = command_line {
                // 播放由 Rust 侧 cpal 负责，页面无需自动播放策略；关掉与本应用无关的功能
                // 窗口被遮挡 / 最小化时不节流：播放中的歌词与进度动画保持更新
                for switch in [
                    "disable-renderer-backgrounding",
                    "disable-background-timer-throttling",
                    "disable-backgrounding-occluded-windows",
                ] {
                    cl.append_switch(Some(&CefString::from(switch)));
                }
                cl.append_switch_with_value(
                    Some(&CefString::from("disable-features")),
                    Some(&CefString::from("Translate,MediaRouter")),
                );
                // macOS：Chromium 默认把 Cookie 加密密钥存进钥匙串，未公证的 .app 会在
                // 浏览器进程与网络服务进程各弹一次“想要访问钥匙串”的密码框。改用本地
                // 派生密钥（use-mock-keychain）；本应用不保存敏感 Cookie，安全性无影响。
                #[cfg(target_os = "macos")]
                cl.append_switch(Some(&CefString::from("use-mock-keychain")));
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(EsBrowserProcessHandler::new(self.shared.clone()))
        }

        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            // 单进程调试等场景下浏览器进程也可能承担渲染
            renderer::renderer_app().render_process_handler()
        }
    }
}

wrap_browser_process_handler! {
    struct EsBrowserProcessHandler {
        shared: Arc<Shared>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            on_context_initialized(&self.shared);
        }
    }
}

// ---------------------------------------------------------------- Client

wrap_client! {
    struct EsClient {
        shared: Arc<Shared>,
    }

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(EsLifeSpanHandler::new(self.shared.clone()))
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(EsRequestHandler::new(self.shared.clone()))
        }

        fn on_process_message_received(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> i32 {
            self.shared
                .router
                .on_process_message_received(
                    browser.cloned(),
                    frame.cloned(),
                    source_process,
                    message.cloned(),
                )
                .into()
        }
    }
}

wrap_life_span_handler! {
    struct EsLifeSpanHandler {
        shared: Arc<Shared>,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let mut slot = self.shared.browser.lock().unwrap();
            if slot.is_none() {
                *slot = browser.cloned();
            }
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let mut browser = browser.cloned();
            self.shared.router.on_before_close(browser.clone());
            let mut slot = self.shared.browser.lock().unwrap();
            let is_main = match (slot.as_ref(), browser.as_mut()) {
                (Some(main), Some(b)) => main.is_same(Some(b)) != 0,
                _ => false,
            };
            if is_main {
                *slot = None;
                quit_message_loop();
            }
        }
    }
}

wrap_request_handler! {
    struct EsRequestHandler {
        shared: Arc<Shared>,
    }

    impl RequestHandler {
        fn on_before_browse(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: i32,
            _is_redirect: i32,
        ) -> i32 {
            // 只允许应用自身页面在窗口内导航
            let url = request.map(|r| CefString::from(&r.url()).to_string()).unwrap_or_default();
            if !url.starts_with(ORIGIN) && !url.starts_with("devtools://") {
                return 1;
            }
            self.shared.router.on_before_browse(browser.cloned(), frame.cloned());
            0
        }

        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _is_navigation: i32,
            _is_download: i32,
            _request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut i32>,
        ) -> Option<ResourceRequestHandler> {
            Some(EsResourceRequestHandler::new(self.shared.clone()))
        }

        fn on_render_process_terminated(
            &self,
            browser: Option<&mut Browser>,
            _status: TerminationStatus,
            _error_code: i32,
            _error_string: Option<&CefString>,
        ) {
            self.shared.router.on_render_process_terminated(browser.cloned());
        }
    }
}

wrap_resource_request_handler! {
    struct EsResourceRequestHandler {
        shared: Arc<Shared>,
    }

    impl ResourceRequestHandler {
        // 注意：cef-rs 的默认实现返回 ReturnValue::default()（= RV_CANCEL），必须显式继续
        fn on_before_resource_load(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            ReturnValue::CONTINUE
        }

        fn resource_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let url = CefString::from(&request?.url()).to_string();
            let handler = resource_for(&self.shared.ui_dir, &url);
            log::debug!("resource {url} -> {}", handler.is_some());
            handler
        }
    }
}

// ---------------------------------------------------------------- Views 窗口

wrap_browser_view_delegate! {
    struct EsBrowserViewDelegate {}

    impl ViewDelegate {}

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_window_delegate! {
    struct EsWindowDelegate {
        browser_view: std::cell::RefCell<Option<BrowserView>>,
        shared: Arc<Shared>,
        size: (i32, i32),
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: self.size.0, height: self.size.1 }
        }

        fn minimum_size(&self, _view: Option<&mut View>) -> Size {
            Size { width: 900, height: 560 }
        }
    }

    impl PanelDelegate {}

    impl WindowDelegate {
        fn on_window_created(&self, window: Option<&mut Window>) {
            let browser_view = self.browser_view.borrow();
            let (Some(window), Some(browser_view)) = (window, browser_view.as_ref()) else {
                return;
            };
            window.set_title(Some(&CefString::from("English Speaker")));
            let mut view = View::from(browser_view);
            window.add_child_view(Some(&mut view));
            window.center_window(Some(&Size { width: self.size.0, height: self.size.1 }));
            *self.shared.window.lock().unwrap() = Some(window.clone());
            apply_drag_regions(window);
            window.show();
            browser_view.request_focus();
        }

        fn on_window_destroyed(&self, _window: Option<&mut Window>) {
            *self.browser_view.borrow_mut() = None;
            *self.shared.window.lock().unwrap() = None;
        }

        fn on_window_bounds_changed(&self, window: Option<&mut Window>, _new_bounds: Option<&Rect>) {
            if let Some(window) = window {
                apply_drag_regions(window);
            }
        }

        fn is_frameless(&self, _window: Option<&mut Window>) -> i32 {
            1
        }

        /// 无边框后不用系统标题栏按钮，关闭 / 最小化 / 最大化由页面顶栏按钮承担
        fn with_standard_window_buttons(&self, _window: Option<&mut Window>) -> i32 {
            0
        }

        fn can_close(&self, _window: Option<&mut Window>) -> i32 {
            let browser_view = self.browser_view.borrow();
            match browser_view.as_ref().and_then(|bv| bv.browser()) {
                Some(browser) => browser.host().map_or(1, |h| h.try_close_browser()),
                None => 1,
            }
        }

        fn window_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

/// 无边框窗口的拖动区：顶部整条可拖动，右侧窗口按钮区域挖孔（页面按钮仍可点击）。
fn apply_drag_regions(window: &mut Window) {
    const TITLEBAR: i32 = 40; // 与 ui/web Titlebar.svelte 高度一致
    const CONTROLS: i32 = 132; // 右侧三个窗口按钮的占位宽度
    let width = window.bounds().width;
    let regions = [
        DraggableRegion {
            bounds: Rect { x: 0, y: 0, width, height: TITLEBAR },
            draggable: 1,
        },
        DraggableRegion {
            bounds: Rect { x: (width - CONTROLS).max(0), y: 0, width: CONTROLS, height: TITLEBAR },
            draggable: 0,
        },
    ];
    window.set_draggable_regions(Some(&regions));
}
