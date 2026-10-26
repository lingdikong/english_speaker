import SwiftUI

/// 启动参数（便于无人工交互截图 / 自动化）：
///   -uiTestScreen home|library|search|settings|player|import   初始页面
///   -theme light|dark          覆盖主题（等同设置里固定）
///   -resetData YES             清空数据后重新预置示例文章
///   -autoplay N                启动后播放资料库第 N 篇（从 0 开始）
///   -autoplayTitle T           启动后播放标题以 T 开头的文章
///   -seekSentence N            播放后跳到第 N 句（等该句合成完）
///   -pauseAfter S              播放 S 秒后暂停
///   -pin N / -removeDownload N 钉住第 N 篇 / 删除第 N 篇的缓存
///   -bar B|C                   真实滚动当前 Tab 进入 B；C 再点左侧小球
///   -focusNow YES              播放页立即进入专注模式
///   -wordSheet N               播放页打开当前句第 N 个词的读音面板
///   -playerSheet reading|queue 播放页打开阅读设置 / 待播清单
///   -importTitle T -importBody B -importCommit YES   导入页预填并自动提交
///   -searchQuery Q             搜索框预填
///   -scrollTo ID               当前 Tab 滚动到指定 id（如设置页的 cache）
///   -demoCollections YES       预置两个分类：“晨间”（前 4 篇）、“睡前故事”（第 2 篇，与晨间重叠）
///   -openCollection NAME       资料库打开名为 NAME 的分类页
@main
struct EnglishSpeakerApp: App {
    @State private var store = Store()
    @State private var prefs = Prefs()
    @State private var ui = AppUI()

    var body: some Scene {
        WindowGroup {
            RootView()
                .themed()
                .environment(store)
                .environment(prefs)
                .environment(ui)
                .task { await bootstrap() }
        }
    }

    @MainActor
    private func bootstrap() async {
        let d = UserDefaults.standard
        if let t = d.string(forKey: "uiTestScreen") {
            switch t {
            case "home": ui.tab = .home
            case "search": ui.tab = .search
            case "settings": ui.tab = .settings
            default: ui.tab = .library
            }
        }
        store.start(resetData: d.bool(forKey: "resetData"))
        if prefs.defaultRate != 1 { store.setRate(Float(prefs.defaultRate)) }
        func int(_ k: String) -> Int? { d.string(forKey: k).flatMap(Int.init) }
        if let i = int("pin"), let a = store.articles[safe: i] { store.setPinned(a.id, true) }
        if let i = int("removeDownload"), let a = store.articles[safe: i] { store.removeDownload(a.id) }
        if d.bool(forKey: "demoCollections"), store.collections.isEmpty {
            let ids = store.articles.map(\.id)
            if let c = store.createCollection("晨间") { ids.prefix(4).forEach { store.toggleCollection(c.id, article: $0) } }
            if let c = store.createCollection("睡前故事"), ids.count > 1 { store.toggleCollection(c.id, article: ids[1]) }
        }
        if let name = d.string(forKey: "openCollection"), let c = store.collections.first(where: { $0.name == name }) {
            ui.tab = .library
            ui.openCollection = c.id
        }
        if d.string(forKey: "uiTestScreen") == "import" {
            try? await Task.sleep(for: .milliseconds(400))
            ui.showImport = true
        }
        let byTitle = d.string(forKey: "autoplayTitle").flatMap { t in store.articles.first { $0.title.hasPrefix(t) } }
        if let a = byTitle ?? int("autoplay").flatMap({ store.articles[safe: $0] }) {
            store.play(a.id, context: store.articles)
            if let s = int("seekSentence") {
                for _ in 0..<300 {
                    if store.timeline?.sentences[safe: s]?.ready == true { break }
                    try? await Task.sleep(for: .milliseconds(100))
                }
                store.seekSentence(s)
            }
            if d.string(forKey: "uiTestScreen") == "player" {
                try? await Task.sleep(for: .milliseconds(300))
                store.isPlayerExpanded = true
            }
            if let s = d.string(forKey: "pauseAfter").flatMap(Double.init) {
                try? await Task.sleep(for: .seconds(s))
                store.pause()
            }
        }
        if let target = d.string(forKey: "scrollTo") {
            try? await Task.sleep(for: .seconds(1))
            ui.scrollRequest = (ui.tab, target)
        }
        if let bar = d.string(forKey: "bar") {
            try? await Task.sleep(for: .seconds(1))
            let rows = store.articles
            if let target = rows[safe: min(3, rows.count - 1)] {
                ui.scrollRequest = (ui.tab, target.id)
            }
            if bar == "C" {
                try? await Task.sleep(for: .seconds(1.2))
                ui.expandTabs()
            }
        }
    }
}
