import SpeakerCore
import SwiftUI
import UIKit

enum AppTab: String, CaseIterable, Hashable {
    case home, library, search, settings

    var title: String {
        switch self { case .home: "首页"; case .library: "资料库"; case .search: "搜索"; case .settings: "设置" }
    }

    var icon: Icon {
        switch self { case .home: .home; case .library: .lib; case .search: .search; case .settings: .gear }
    }
}

/// 底部栏三态（UI_SPEC §4.2）。
enum BarState { case a, b, c }

/// 界面状态：当前 Tab、底部栏状态、各 Tab 的滚动位置、Sheet。
@MainActor
@Observable
final class AppUI {
    var tab: AppTab = .library
    var bar: BarState = .a
    var showImport = false
    /// 每个 Tab 自己的滚动容器的偏移。
    @ObservationIgnored private var offsets: [AppTab: CGFloat] = [:]
    /// 调试：请求某个 Tab 滚动到指定行（截图 / 自动化）。
    var scrollRequest: (tab: AppTab, id: String)?

    /// 滚动驱动：滚回顶部（< 4）→ A；A 下向下滚超过 24 → B；B、C 下继续滚动保持不变。
    func scrolled(_ t: AppTab, _ y: CGFloat) {
        offsets[t] = y
        guard t == tab else { return }
        if y < 4 {
            if bar != .a { bar = .a }
        } else if bar == .a && y > 24 {
            bar = .b
        }
    }

    /// B 下点左侧小球 → C（无播放条时直接展开导航）。
    func expandTabs() { bar = .c }
    /// C 下点右侧小球 → B。
    func expandPlayer() { bar = .b }

    func select(_ t: AppTab) {
        tab = t
        let y = offsets[t] ?? 0
        if y < 4 { bar = .a } else if bar == .a { bar = .b }
    }
}

struct RootView: View {
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui
    @Environment(Prefs.self) private var prefs
    @Environment(\.palette) private var p

    var body: some View {
        @Bindable var store = store
        @Bindable var ui = ui
        ZStack {
            p.bg.ignoresSafeArea()
            // 四个 Tab 常驻，切换时保留各自的滚动位置
            ForEach(AppTab.allCases, id: \.self) { t in
                tabContent(t)
                    .opacity(t == ui.tab ? 1 : 0)
                    .allowsHitTesting(t == ui.tab)
                    .accessibilityHidden(t != ui.tab)
            }
            // 状态栏下的渐隐：滚动的列表不和时间/电量叠在一起
            GeometryReader { g in
                LinearGradient(stops: [.init(color: p.bg, location: 0), .init(color: p.bg, location: 0.55),
                                       .init(color: p.bg.opacity(0), location: 1)],
                               startPoint: .top, endPoint: .bottom)
                    .frame(height: g.safeAreaInsets.top + 14)
                    .ignoresSafeArea(edges: .top)
            }
            .allowsHitTesting(false)
            BottomBar()
        }
        .fullScreenCover(isPresented: $store.isPlayerExpanded) {
            PlayerView().themed()
        }
        .sheet(isPresented: $ui.showImport) {
            ImportSheet().themed()
        }
        .overlay {
            if let error = store.startupError {
                VStack(spacing: 12) {
                    Text("核心库启动失败").font(Fonts.display(20))
                    Text(error).font(Fonts.ui(13)).foregroundStyle(p.t2)
                }
                .padding(32)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(p.bg)
            }
        }
        .onAppear { applyTheme(prefs.theme) }
        .onChange(of: prefs.theme) { _, t in applyTheme(t) }
    }

    @ViewBuilder
    private func tabContent(_ t: AppTab) -> some View {
        switch t {
        case .home: HomeView()
        case .library: LibraryView()
        case .search: SearchView()
        case .settings: SettingsView()
        }
    }

    /// 主题覆盖：直接设置窗口的 overrideUserInterfaceStyle（Sheet / 全屏页一起生效，“跟随系统”可正确恢复）。
    private func applyTheme(_ t: ThemeMode) {
        let style: UIUserInterfaceStyle = switch t { case .system: .unspecified; case .light: .light; case .dark: .dark }
        for scene in UIApplication.shared.connectedScenes {
            guard let ws = scene as? UIWindowScene else { continue }
            for w in ws.windows { w.overrideUserInterfaceStyle = style }
        }
    }
}

// MARK: - Tab 的滚动容器（驱动底部栏状态 + 玻璃下渐隐）

struct TabScroll<Content: View>: View {
    let tab: AppTab
    @ViewBuilder var content: Content
    @Environment(AppUI.self) private var ui
    @Environment(Store.self) private var store

    var body: some View {
        let space = "tab-scroll-\(tab.rawValue)"
        ScrollViewReader { proxy in
            ScrollView(showsIndicators: false) {
                content
                    .padding(.horizontal, 20)
                    .padding(.top, 8)
                    .padding(.bottom, 190)
                    .onGeometryChange(for: CGFloat.self) { g in
                        -g.frame(in: .named(space)).minY
                    } action: { y in
                        ui.scrolled(tab, y)
                    }
            }
            .coordinateSpace(name: space)
            .onChange(of: ui.scrollRequest?.id) { _, id in
                guard let id, ui.scrollRequest?.tab == tab else { return }
                withAnimation(.easeInOut(duration: 0.5)) { proxy.scrollTo(id, anchor: .top) }
                ui.scrollRequest = nil
            }
        }
        .fadeUnder(ui.bar == .a && store.barArticle != nil ? .twoRows : .oneRow)
    }
}
