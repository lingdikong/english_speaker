//! macOS：CEF 要求 NSApplication 子类实现 CefAppProtocol；另建应用菜单（退出 / 编辑：复制粘贴等快捷键依赖菜单）。

use std::cell::Cell;

use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, NSObject, NSObjectProtocol, ProtocolObject, Sel};
use objc2::{
    define_class, extern_methods, msg_send, sel, ClassType, DefinedClass, MainThreadMarker,
    MainThreadOnly,
};
use objc2_app_kit::{
    NSApp, NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSApplicationTerminateReply, NSEvent, NSEventModifierFlags, NSMenu, NSMenuItem,
};
use objc2_foundation::NSString;

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct EsAppDelegate;

    unsafe impl NSObjectProtocol for EsAppDelegate {}

    unsafe impl NSApplicationDelegate for EsAppDelegate {
        #[unsafe(method(applicationShouldTerminate:))]
        fn should_terminate(&self, _sender: &NSApplication) -> NSApplicationTerminateReply {
            NSApplicationTerminateReply::TerminateNow
        }

        #[unsafe(method(applicationSupportsSecureRestorableState:))]
        fn secure_restorable(&self, _sender: &NSApplication) -> Bool {
            Bool::YES
        }
    }
);

#[derive(Default)]
struct AppIvars {
    handling_send_event: Cell<Bool>,
}

define_class!(
    /// 实现 CEF 所需协议的 NSApplication 子类。
    #[unsafe(super(NSApplication))]
    #[ivars = AppIvars]
    struct EsApplication;

    impl EsApplication {
        #[unsafe(method(sendEvent:))]
        fn send_event(&self, event: &NSEvent) {
            let was = self.ivars().handling_send_event.get();
            self.ivars().handling_send_event.set(Bool::YES);
            let _: () = unsafe { msg_send![super(self), sendEvent: event] };
            self.ivars().handling_send_event.set(was);
        }

        /// ⌘Q：先让 CEF 有序关闭浏览器，再退出消息循环（不能直接 exit）。
        #[unsafe(method(terminate:))]
        fn terminate(&self, _sender: Option<&AnyObject>) {
            match super::shared() {
                Some(shared) => shared.close_all(),
                None => cef::quit_message_loop(),
            }
        }
    }

    unsafe impl CrAppControlProtocol for EsApplication {
        #[unsafe(method(setHandlingSendEvent:))]
        fn set_handling(&self, v: Bool) {
            self.ivars().handling_send_event.set(v);
        }
    }

    unsafe impl CrAppProtocol for EsApplication {
        #[unsafe(method(isHandlingSendEvent))]
        fn is_handling(&self) -> Bool {
            self.ivars().handling_send_event.get()
        }
    }

    unsafe impl CefAppProtocol for EsApplication {}
);

impl EsApplication {
    extern_methods! {
        #[unsafe(method(sharedApplication))]
        fn shared_application() -> Retained<Self>;
    }
}

/// 必须在任何 NSApp 调用之前执行。
pub fn setup_application() {
    let _ = EsApplication::shared_application();
    let mtm = MainThreadMarker::new().expect("main thread");
    assert!(NSApp(mtm).isKindOfClass(EsApplication::class()));
}

pub fn setup_delegate() -> Retained<ProtocolObject<dyn NSApplicationDelegate>> {
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApp(mtm);
    let delegate: Retained<EsAppDelegate> = unsafe { msg_send![EsAppDelegate::alloc(mtm), init] };
    let delegate = ProtocolObject::from_retained(delegate);
    app.setDelegate(Some(&delegate));
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    app.setMainMenu(Some(&main_menu(mtm)));
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    delegate
}

fn item(
    mtm: MainThreadMarker,
    title: &str,
    action: Option<Sel>,
    key: &str,
) -> Retained<NSMenuItem> {
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key),
        )
    }
}

fn submenu(
    mtm: MainThreadMarker,
    title: &str,
    items: Vec<Retained<NSMenuItem>>,
) -> Retained<NSMenuItem> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
    for it in items {
        menu.addItem(&it);
    }
    let top = item(mtm, title, None, "");
    top.setSubmenu(Some(&menu));
    top
}

/// 应用菜单 + 编辑菜单（复制、粘贴、全选等走响应链）+ 窗口菜单。
fn main_menu(mtm: MainThreadMarker) -> Retained<NSMenu> {
    let bar = NSMenu::new(mtm);
    let hide_others = item(mtm, "隐藏其他", Some(sel!(hideOtherApplications:)), "h");
    hide_others
        .setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Option);
    bar.addItem(&submenu(
        mtm,
        "English Speaker",
        vec![
            item(mtm, "隐藏 English Speaker", Some(sel!(hide:)), "h"),
            hide_others,
            NSMenuItem::separatorItem(mtm),
            item(mtm, "退出 English Speaker", Some(sel!(terminate:)), "q"),
        ],
    ));
    let redo = item(mtm, "重做", Some(sel!(redo:)), "z");
    redo.setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Shift);
    bar.addItem(&submenu(
        mtm,
        "编辑",
        vec![
            item(mtm, "撤销", Some(sel!(undo:)), "z"),
            redo,
            NSMenuItem::separatorItem(mtm),
            item(mtm, "剪切", Some(sel!(cut:)), "x"),
            item(mtm, "拷贝", Some(sel!(copy:)), "c"),
            item(mtm, "粘贴", Some(sel!(paste:)), "v"),
            item(mtm, "全选", Some(sel!(selectAll:)), "a"),
        ],
    ));
    bar.addItem(&submenu(
        mtm,
        "窗口",
        vec![
            item(mtm, "最小化", Some(sel!(performMiniaturize:)), "m"),
            item(mtm, "缩放", Some(sel!(performZoom:)), ""),
            item(mtm, "关闭窗口", Some(sel!(performClose:)), "w"),
        ],
    ));
    bar
}
