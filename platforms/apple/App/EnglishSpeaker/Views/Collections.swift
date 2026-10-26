import SpeakerCore
import SwiftUI

// 分类（同歌单）：资料库里的分类卡片、分类页、命名 / 删除对话框。一篇文章可以同时在多个分类里。

/// 分类封面：4 篇及以上拼成 2×2，1–3 篇用第一篇的封面，空分类用分类名生成的 violet 声波。
struct CollectionCover: View {
    let collection: ArticleCollection
    let size: CGFloat
    @Environment(Store.self) private var store

    var body: some View {
        let items = store.articles(in: collection.id)
        if items.count >= 4 {
            let gap = max(2, (size / 40).rounded())
            let half = ((size - gap) / 2).rounded(.down)
            VStack(spacing: gap) {
                ForEach(0..<2, id: \.self) { r in
                    HStack(spacing: gap) {
                        ForEach(0..<2, id: \.self) { c in
                            let i = r * 2 + c
                            WaveCover(title: items[i].title, size: half, violet: i == 1 || i == 2)
                        }
                    }
                }
            }
            .frame(width: size, height: size)
            .clipShape(RoundedRectangle(cornerRadius: (size / 4.5).rounded(), style: .continuous))
            .accessibilityHidden(true)
        } else if let first = items.first {
            WaveCover(title: first.title, size: size)
        } else {
            WaveCover(title: collection.name, size: size, violet: true)
        }
    }
}

/// 资料库“全部”下的分类：新建分类 + 每个分类一张卡片，横向滚动。长按卡片：播放全部 / 重命名 / 删除。
struct CollectionShelf: View {
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui
    @Environment(\.palette) private var p
    @Environment(\.accessibilityReduceMotion) private var reduce

    private let cover: CGFloat = 132

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("分类").font(Fonts.display(20, .semibold))
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(alignment: .top, spacing: 14) {
                    Button { ui.namePrompt = .create(articleId: nil) } label: {
                        card(name: "新建分类", detail: "像歌单一样整理") {
                            RoundedRectangle(cornerRadius: (cover / 4.5).rounded(), style: .continuous)
                                .strokeBorder(p.waveB, style: StrokeStyle(lineWidth: 1.5, dash: [5, 4]))
                                .background(p.surface, in: RoundedRectangle(cornerRadius: (cover / 4.5).rounded(), style: .continuous))
                                .overlay { IconView(.plus, size: 30, weight: 2).foregroundStyle(p.t2) }
                                .frame(width: cover, height: cover)
                        }
                    }
                    .buttonStyle(PressStyle())
                    .accessibilityIdentifier("collection-new")
                    ForEach(store.collections, id: \.id) { c in
                        let n = c.articleIds.count
                        Button {
                            withAnimation(Motion.spring(reduce)) { ui.openCollection = c.id }
                        } label: {
                            card(name: c.name, detail: n > 0 ? "\(n) 篇" : "空分类") {
                                CollectionCover(collection: c, size: cover)
                            }
                        }
                        .buttonStyle(PressStyle())
                        .contextMenu { CollectionMenu(collection: c) }
                        .accessibilityLabel("\(c.name)，\(n) 篇")
                        .accessibilityIdentifier("collection-\(c.name)")
                    }
                }
                .padding(.horizontal, 20)
            }
            .padding(.horizontal, -20)
        }
    }

    private func card<C: View>(name: String, detail: String, @ViewBuilder cover: () -> C) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            cover().padding(.bottom, 5)
            Text(name).font(Fonts.ui(14, .bold)).foregroundStyle(p.t1).lineLimit(1)
            Text(detail).font(Fonts.ui(12, .medium)).foregroundStyle(p.t2)
        }
        .frame(width: self.cover, alignment: .leading)
        .contentShape(Rectangle())
    }
}

/// 分类的更多操作（长按卡片 / 分类页 ⋯）。
struct CollectionMenu: View {
    let collection: ArticleCollection
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui

    var body: some View {
        Button { store.playCollection(collection.id) } label: { Text("播放全部") }
            .disabled(collection.articleIds.isEmpty)
        Button { ui.namePrompt = .rename(id: collection.id) } label: { Text("重命名") }
        Divider()
        Button(role: .destructive) { ui.confirmDelete = collection.id } label: { Text("删除分类") }
    }
}

/// 分类页：从右侧推入，左上角返回或从左边缘右滑返回。大封面 + 名称 + 篇数 / 时长 · 播放全部 · ⋯ · 文章列表。
struct CollectionView: View {
    let collection: ArticleCollection
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui
    @Environment(\.palette) private var p
    @Environment(\.accessibilityReduceMotion) private var reduce
    /// 边缘右滑返回时页面跟手的位移。
    @State private var drag: CGFloat = 0

    var body: some View {
        let items = store.articles(in: collection.id)
        let total = items.reduce(UInt64(0)) { $0 + $1.durationMs }
        let playing = store.isPlaying && items.contains { $0.id == store.player.articleId }
        TabScroll(tab: .library) {
            VStack(alignment: .leading, spacing: 20) {
                HStack {
                    IconButton(icon: .down, size: 22, weight: 2.2, fill: true, label: "返回资料库") { close() }
                        .rotationEffect(.degrees(90))
                    Spacer()
                    Menu { CollectionMenu(collection: collection) } label: {
                        MoreDots().frame(width: 22, height: 22).foregroundStyle(p.t1)
                            .frame(width: 44, height: 44).background(p.control, in: Circle())
                    }
                    .accessibilityLabel("更多操作")
                }
                VStack(spacing: 14) {
                    CollectionCover(collection: collection, size: 200)
                        .shadow(color: .black.opacity(p.isDark ? 0.35 : 0.12), radius: 18, y: 10)
                    VStack(spacing: 4) {
                        Text(collection.name).font(Fonts.display(26)).foregroundStyle(p.t1)
                            .multilineTextAlignment(.center).lineLimit(2)
                        Text(items.isEmpty ? "空分类" : "\(items.count) 篇 · \(Fmt.minutes(total))")
                            .font(Fonts.ui(14, .medium)).foregroundStyle(p.t2)
                    }
                    Button {
                        if playing { store.pause() } else { store.playCollection(collection.id) }
                    } label: {
                        HStack(spacing: 8) {
                            PlayGlyph(playing: playing, size: 16)
                            Text(playing ? "暂停" : "播放全部").font(Fonts.ui(15, .bold))
                        }
                        .foregroundStyle(p.accentInk)
                        .frame(width: 180, height: 48)
                        .background(p.accent.opacity(items.isEmpty ? 0.45 : 1), in: Capsule())
                    }
                    .buttonStyle(PressStyle())
                    .disabled(items.isEmpty)
                }
                .frame(maxWidth: .infinity)
                VStack(alignment: .leading, spacing: 0) {
                    if items.isEmpty {
                        Text("在文章的 ⋯ 菜单里选“添加到分类”，就能把文章放进来。一篇文章可以放进多个分类。")
                            .font(Fonts.ui(14)).foregroundStyle(p.t2).multilineTextAlignment(.center)
                            .frame(maxWidth: .infinity).padding(.vertical, 28)
                    }
                    ForEach(Array(items.enumerated()), id: \.element.id) { i, a in
                        ArticleRow(article: a, index: i, context: items, inCollection: collection.id).id(a.id)
                    }
                }
            }
        }
        .background(p.bg)
        .offset(x: drag)
        .overlay(alignment: .leading) {
            // 左边缘右滑返回（同系统导航）
            Color.clear.frame(width: 20).contentShape(Rectangle())
                .gesture(
                    DragGesture(minimumDistance: 10)
                        .onChanged { drag = max(0, $0.translation.width) }
                        .onEnded { v in
                            if v.translation.width > 100 || v.predictedEndTranslation.width > 240 { close() }
                            else { withAnimation(Motion.spring(reduce)) { drag = 0 } }
                        }
                )
        }
    }

    private func close() {
        withAnimation(Motion.spring(reduce)) { ui.openCollection = nil }
    }
}

/// 分类的命名（新建 / 重命名）与删除确认，挂在根视图上。
struct CollectionDialogs: ViewModifier {
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui

    func body(content: Content) -> some View {
        @Bindable var ui = ui
        let prompt = ui.namePrompt
        content
            .alert(title(prompt), isPresented: Binding(get: { ui.namePrompt != nil }, set: { if !$0 { ui.namePrompt = nil } })) {
                TextField("分类名称", text: $ui.promptText)
                Button(prompt.map { if case .rename = $0 { "保存" } else { "创建" } } ?? "创建") { submit(prompt) }
                Button("取消", role: .cancel) { ui.promptError = nil }
            } message: {
                Text(ui.promptError ?? message(prompt))
            }
            .confirmationDialog(
                deleteTitle, isPresented: Binding(get: { ui.confirmDelete != nil }, set: { if !$0 { ui.confirmDelete = nil } }),
                titleVisibility: .visible
            ) {
                Button("删除分类", role: .destructive) {
                    if let id = ui.confirmDelete {
                        if ui.openCollection == id { ui.openCollection = nil }
                        store.deleteCollection(id)
                    }
                }
            } message: {
                Text("其中的文章不会被删除，只是不再属于这个分类。")
            }
            .onChange(of: prompt) { _, p in
                // 打开对话框时预填：重命名填当前名称，新建为空（出错重新弹出时保留输入）
                guard let p, ui.promptError == nil else { return }
                if case let .rename(id) = p { ui.promptText = store.collection(id)?.name ?? "" } else { ui.promptText = "" }
            }
    }

    private var deleteTitle: String {
        ui.confirmDelete.flatMap(store.collection).map { "删除“\($0.name)”？" } ?? "删除分类？"
    }

    private func title(_ p: CollectionPrompt?) -> String {
        if case .rename = p { return "重命名分类" }
        return "新建分类"
    }

    private func message(_ p: CollectionPrompt?) -> String {
        switch p {
        case .create(let id?) where !id.isEmpty: "建好后会把这篇文章加进去。"
        case .rename: ""
        default: "把文章整理到分类里，像歌单一样一起听。"
        }
    }

    /// 校验失败时带着错误提示重新弹出（系统对话框点按钮后总会关闭）。
    private func submit(_ p: CollectionPrompt?) {
        guard let p else { return }
        let name = ui.promptText.trimmingCharacters(in: .whitespacesAndNewlines)
        var except: String?
        if case let .rename(id) = p { except = id }
        if let error = store.collectionNameError(name, except: except) {
            ui.promptError = error
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(350))
                ui.namePrompt = p
            }
            return
        }
        ui.promptError = nil
        switch p {
        case .create(let articleId): store.createCollection(name, adding: articleId)
        case .rename(let id): store.renameCollection(id, to: name)
        }
    }
}

/// 命名对话框的用途。
enum CollectionPrompt: Equatable {
    /// 新建；给了 articleId 时建好后把这篇加进去。
    case create(articleId: String?)
    case rename(id: String)
}
