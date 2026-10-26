import SpeakerCore
import SwiftUI

enum LibraryFilter: CaseIterable { case all, converting, saved, notConverted
    func matches(_ a: ArticleSummary) -> Bool {
        switch self {
        case .all: true
        case .converting: [.queued, .synthesizing, .partial].contains(a.status)
        case .saved: a.pinned
        case .notConverted: a.status == .notCached || a.status == .failed
        }
    }
}

/// 资料库（UI_SPEC §4.4）：大标题 + 搜索/导入；筛选 chip；分类（“全部”下）；继续收听；全部文章。
/// 点分类卡片从右侧推入分类页。
struct LibraryView: View {
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui
    @Environment(\.palette) private var p
    @State private var filter: LibraryFilter = .all

    var body: some View {
        ZStack {
            library
                .allowsHitTesting(ui.openCollection == nil)
                .accessibilityHidden(ui.openCollection != nil)
            if let id = ui.openCollection, let c = store.collection(id) {
                CollectionView(collection: c)
                    .transition(.move(edge: .trailing))
                    .zIndex(1)
            }
        }
        // 分类被删除（或在别处消失）时退回资料库
        .onChange(of: store.collections.map(\.id)) { _, ids in
            if let id = ui.openCollection, !ids.contains(id) { ui.openCollection = nil }
        }
    }

    private var library: some View {
        TabScroll(tab: .library) {
            VStack(alignment: .leading, spacing: 20) {
                HStack {
                    LargeTitle("资料库")
                    Spacer()
                    HStack(spacing: 8) {
                        IconButton(icon: .search, fill: true, label: "搜索") { ui.select(.search) }
                        Button { ui.showImport = true } label: {
                            IconView(.plus, size: 22, weight: 2.2)
                                .foregroundStyle(p.accentInk)
                                .frame(width: 44, height: 44)
                                .background(p.accent, in: Circle())
                        }
                        .buttonStyle(PressStyle())
                        .accessibilityLabel("导入文章")
                        .accessibilityIdentifier("import-button")
                    }
                }
                filters
                if filter == .all && !store.articles.isEmpty { CollectionShelf() }
                if let a = store.barArticle { ContinueCard(article: a) }
                VStack(alignment: .leading, spacing: 0) {
                    Text("全部文章").font(Fonts.display(20, .semibold)).padding(.bottom, 4)
                    let items = store.articles.filter(filter.matches)
                    if items.isEmpty {
                        Text(store.articles.isEmpty ? "还没有文章，点右上角 + 导入" : "这里还没有文章")
                            .font(Fonts.ui(14)).foregroundStyle(p.t2).padding(.vertical, 28)
                    }
                    ForEach(Array(items.enumerated()), id: \.element.id) { i, a in
                        ArticleRow(article: a, index: i, context: items, showCollections: true).id(a.id)
                    }
                }
            }
        }
    }

    private var filters: some View {
        let converting = store.articles.filter(LibraryFilter.converting.matches).count
        return ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                chip("全部 \(store.articles.count)", .all)
                chip(converting > 0 ? "转换中 \(converting)" : "转换中", .converting)
                chip("已保存", .saved)
                chip("未转换", .notConverted)
            }
        }
    }

    private func chip(_ title: String, _ f: LibraryFilter) -> some View {
        Button { filter = f } label: {
            Chip(title, kind: filter == f ? .accent : .muted, height: 34, fontSize: 14)
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(filter == f ? .isSelected : [])
    }
}

/// 资料库行：56 封面 · 标题 15/700 · “时长 · 音色” + 状态 chip · 更多。
struct ArticleRow: View {
    let article: ArticleSummary
    let index: Int
    var context: [ArticleSummary]? = nil
    var titleText: AttributedString? = nil
    var snippet: AttributedString? = nil
    /// 在该分类页里：菜单多一项“从此分类移除”。
    var inCollection: String? = nil
    /// 标题下注明所在分类（资料库“全部文章”）。
    var showCollections = false
    @Environment(Store.self) private var store
    @Environment(\.palette) private var p

    var body: some View {
        HStack(spacing: 14) {
            WaveCover(title: article.title, size: 56, violet: index % 2 == 0)
            VStack(alignment: .leading, spacing: 0) {
                Group {
                    if let titleText { Text(titleText) } else { Text(article.title) }
                }
                .font(Fonts.ui(15, .bold))
                .foregroundStyle(article.id == store.player.articleId ? p.accentText : p.t1)
                .lineSpacing(2)
                .lineLimit(2)
                if let snippet {
                    Text(snippet).font(Fonts.ui(13, .medium)).foregroundStyle(p.t2).lineLimit(2).padding(.top, 4)
                }
                if showCollections, case let cols = store.collections(of: article.id), !cols.isEmpty {
                    HStack(spacing: 4) {
                        IconView(.lib, size: 11, weight: 2)
                        Text(cols.map(\.name).joined(separator: " · ")).lineLimit(1)
                    }
                    .font(Fonts.ui(12, .semibold)).foregroundStyle(p.violetText).padding(.top, 4)
                }
                HStack(spacing: 8) {
                    Text(store.rowMeta(article)).font(Fonts.ui(13, .medium)).foregroundStyle(p.t2).lineLimit(1)
                    StatusChip(article: article)
                }
                .padding(.top, 6)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Menu {
                ArticleMenu(article: article, inCollection: inCollection)
            } label: {
                MoreDots().frame(width: 22, height: 22).foregroundStyle(p.t2)
                    .frame(width: 44, height: 44).contentShape(Circle())
            }
            .accessibilityLabel("更多操作")
        }
        .padding(.vertical, 12)
        .overlay(alignment: .bottom) { Rectangle().fill(p.line).frame(height: 1) }
        .contentShape(Rectangle())
        .onTapGesture {
            store.play(article.id, context: context ?? store.articles)
            store.isPlayerExpanded = true
        }
        .contextMenu { ArticleMenu(article: article, inCollection: inCollection) }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        // 分类页盖在资料库列表上：两层的行用不同前缀，自动化测试不会点到下面那层
        .accessibilityIdentifier(inCollection == nil ? "row-\(index)" : "collection-row-\(index)")
    }
}

/// 长按 / ⋯：保存（钉住）/ 取消保存、换音色、添加到分类、加入待播、删除缓存、删除文章。
struct ArticleMenu: View {
    let article: ArticleSummary
    var inCollection: String? = nil
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui

    var body: some View {
        Button { store.setPinned(article.id, !article.pinned) } label: {
            Text(article.pinned ? "取消保存" : "保存（离线保留）")
        }
        Menu("换音色") {
            ForEach(store.voices, id: \.id) { v in
                Button {
                    store.synthesize(article.id, voice: v.id)
                } label: {
                    if (article.voiceId ?? Store.defaultVoice) == v.id {
                        Label(Fmt.voice(v), systemImage: "checkmark")
                    } else {
                        Text(Fmt.voice(v))
                    }
                }
            }
        }
        // 已在其中的打 ✓，再点一次即移出
        Menu("添加到分类") {
            ForEach(store.collections, id: \.id) { c in
                Button { store.toggleCollection(c.id, article: article.id) } label: {
                    if c.articleIds.contains(article.id) {
                        Label(c.name, systemImage: "checkmark")
                    } else {
                        Text(c.name)
                    }
                }
            }
            Divider()
            Button { ui.namePrompt = .create(articleId: article.id) } label: { Label("新建分类…", systemImage: "plus") }
        }
        if let inCollection {
            Button { store.removeFromCollection(inCollection, article: article.id) } label: { Text("从此分类移除") }
        }
        Button { store.addToQueue(article.id, next: false) } label: { Text("加入待播清单") }
        if article.status != .notCached && article.id != store.player.articleId {
            Button { store.removeDownload(article.id) } label: { Text("删除缓存") }
        }
        Divider()
        Button(role: .destructive) { store.delete(article.id) } label: { Text("删除文章") }
    }
}

/// “继续收听”卡片：surface 面板，72 封面 + 标题 18 + “第 x/y 句 · 剩余 N 分钟” + 48 播放键 + 4px 进度条。
struct ContinueCard: View {
    let article: ArticleSummary
    @Environment(Store.self) private var store
    @Environment(\.palette) private var p

    var body: some View {
        let (i, n) = store.sentenceProgress(article)
        let isCurrent = article.id == store.player.articleId
        Panel(radius: 22, padding: 16) {
            VStack(alignment: .leading, spacing: 14) {
                LabelText("继续收听")
                HStack(spacing: 14) {
                    WaveCover(title: article.title, size: 72)
                    VStack(alignment: .leading, spacing: 4) {
                        Text(article.title).font(Fonts.display(18)).lineSpacing(0).lineLimit(2)
                        Text("第 \(i)/\(max(n, 1)) 句 · 剩余 \(Fmt.minutes(store.remainingMs(article)))")
                            .font(Fonts.ui(13, .medium)).foregroundStyle(p.t2)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    PlayButton(playing: isCurrent && store.isPlaying, diameter: 48, glyph: 20,
                               buffering: isCurrent && store.player.status == .buffering) {
                        store.playOrToggle(article.id)
                    }
                }
                GeometryReader { g in
                    ZStack(alignment: .leading) {
                        Capsule().fill(p.waveE)
                        Capsule().fill(p.accent).frame(width: g.size.width * store.progressFraction(article))
                    }
                }
                .frame(height: 4)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture {
            if !isCurrent { store.play(article.id, context: store.articles) }
            store.isPlayerExpanded = true
        }
    }
}
