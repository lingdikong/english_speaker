//! CEF 渲染进程：注册 message router 的 JS 端（`window.cefQuery`）。
//!
//! macOS 由 `es-desktop-helper` 引入（`#[path]`），Windows / Linux 由主程序在 `--type=` 子进程中使用。

use std::sync::Arc;

use cef::wrapper::message_router::{
    MessageRouterConfig, MessageRouterRendererSide, MessageRouterRendererSideHandlerCallbacks,
    RendererSideRouter,
};
use cef::*;

wrap_app! {
    pub struct RendererApp {
        router: Arc<RendererSideRouter>,
    }

    impl App {
        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(EsRenderProcessHandler::new(self.router.clone()))
        }
    }
}

wrap_render_process_handler! {
    struct EsRenderProcessHandler {
        router: Arc<RendererSideRouter>,
    }

    impl RenderProcessHandler {
        fn on_context_created(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            self.router
                .on_context_created(browser.cloned(), frame.cloned(), context.cloned());
        }

        fn on_context_released(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            self.router
                .on_context_released(browser.cloned(), frame.cloned(), context.cloned());
        }

        fn on_process_message_received(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> i32 {
            self.router
                .on_process_message_received(
                    browser.cloned(),
                    frame.cloned(),
                    Some(source_process),
                    message.cloned(),
                )
                .into()
        }
    }
}

pub fn renderer_app() -> App {
    RendererApp::new(RendererSideRouter::new(MessageRouterConfig::default()))
}

/// 子进程入口：执行 CEF 子进程逻辑，返回退出码。
// macOS 主程序不直接用（由 helper 调用）
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn run_subprocess(args: &cef::args::Args) -> i32 {
    let mut app = renderer_app();
    execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    )
}
