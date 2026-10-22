import SpeakerCore
import SwiftUI

/// 底部栏三态（UI_SPEC §4.2）：两个玻璃药丸——播放条与导航。
///
/// | 状态 | 布局 |
/// |---|---|
/// | A | 两行：播放条 bottom 96、导航 bottom 26，都占满宽度 |
/// | B | 一行：导航收成左侧 60×60 小球；播放条 left 86 占满剩余 |
/// | C | 一行：导航展开（right 86）；播放条收成右侧 60×60 小球 |
///
/// 位置/宽度用弹簧（0.42s），内部内容 0.2s 淡入淡出；减少动态效果时直接切换。
struct BottomBar: View {
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui
    @Environment(\.accessibilityReduceMotion) private var reduce

    static let height: CGFloat = 60
    static let side: CGFloat = 16
    static let bottom: CGFloat = 26
    static let gap: CGFloat = 10

    var body: some View {
        GeometryReader { geo in
            let w = geo.size.width
            let h = geo.size.height
            let article = store.barArticle
            let state = ui.bar
            let tabs = tabsFrame(state, width: w, hasPlayer: article != nil)
            ZStack(alignment: .topLeading) {
                if let article {
                    let pf = playerFrame(state, width: w)
                    PlayerPill(article: article, state: state, fullWidth: w - 2 * Self.side)
                        .frame(width: pf.width, height: Self.height, alignment: .leading)
                        .glass(Capsule())
                        .clipShape(Capsule())
                        .position(x: pf.midX, y: h - pf.bottom - Self.height / 2)
                        .transition(.opacity)
                }
                TabsPill(state: state, collapsed: state == .b)
                    .frame(width: tabs.width, height: Self.height, alignment: .leading)
                    .glass(Capsule())
                    .clipShape(Capsule())
                    .position(x: tabs.midX, y: h - Self.bottom - Self.height / 2)
            }
            .animation(Motion.spring(reduce), value: state)
            .animation(Motion.fade(reduce), value: article?.id)
        }
        .ignoresSafeArea(.container, edges: .bottom)
        .ignoresSafeArea(.keyboard)
    }

    private struct Slot { let minX: CGFloat; let maxX: CGFloat; let bottom: CGFloat
        var width: CGFloat { maxX - minX }
        var midX: CGFloat { (minX + maxX) / 2 }
    }

    private func tabsFrame(_ s: BarState, width w: CGFloat, hasPlayer: Bool) -> Slot {
        switch s {
        case .a: Slot(minX: Self.side, maxX: w - Self.side, bottom: Self.bottom)
        case .b: Slot(minX: Self.side, maxX: Self.side + Self.height, bottom: Self.bottom)
        case .c: Slot(minX: Self.side, maxX: hasPlayer ? w - Self.side - Self.height - Self.gap : w - Self.side,
                      bottom: Self.bottom)
        }
    }

    private func playerFrame(_ s: BarState, width w: CGFloat) -> Slot {
        let ball = Self.side + Self.height + Self.gap // 86
        switch s {
        case .a: return Slot(minX: Self.side, maxX: w - Self.side, bottom: Self.bottom + Self.height + Self.gap)
        case .b: return Slot(minX: ball, maxX: w - Self.side, bottom: Self.bottom)
        case .c: return Slot(minX: w - Self.side - Self.height, maxX: w - Self.side, bottom: Self.bottom)
        }
    }
}

// MARK: - 导航药丸

private struct TabsPill: View {
    let state: BarState
    let collapsed: Bool
    @Environment(AppUI.self) private var ui
    @Environment(\.palette) private var p
    @Environment(\.accessibilityReduceMotion) private var reduce

    var body: some View {
        ZStack(alignment: .leading) {
            HStack(spacing: 0) {
                ForEach(AppTab.allCases, id: \.self) { t in
                    let on = t == ui.tab
                    Button {
                        ui.select(t)
                    } label: {
                        VStack(spacing: 3) {
                            IconView(t.icon, size: 22, weight: on ? 2 : 1.75)
                            Text(t.title).font(Fonts.ui(11, .bold)).lineLimit(1)
                        }
                        .foregroundStyle(on ? p.accentText : p.t2)
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .background(on ? p.accentSoft : .clear, in: Capsule())
                        .padding(.vertical, 6).padding(.horizontal, 1)
                        .contentShape(Capsule())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(t.title)
                    .accessibilityAddTraits(on ? .isSelected : [])
                    .accessibilityIdentifier("tab-\(t.rawValue)")
                }
            }
            .padding(.horizontal, 6)
            .opacity(collapsed ? 0 : 1)
            .allowsHitTesting(!collapsed)

            // B：左侧小球，显示当前 Tab 图标
            Button { ui.expandTabs() } label: {
                IconView(ui.tab.icon, size: 24, weight: 2)
                    .foregroundStyle(p.accentText)
                    .frame(width: 60, height: 60)
                    .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .opacity(collapsed ? 1 : 0)
            .allowsHitTesting(collapsed)
            .accessibilityLabel("展开导航（当前：\(ui.tab.title)）")
            .accessibilityHidden(!collapsed)
            .accessibilityIdentifier("bar-tabs-ball")
        }
        .animation(Motion.fade(reduce), value: collapsed)
    }
}

// MARK: - 播放条药丸

private struct PlayerPill: View {
    let article: ArticleSummary
    let state: BarState
    /// 内容按满宽排版，药丸变窄时裁切（收成小球时正好剩左端圆形封面 + 进度环）。
    let fullWidth: CGFloat
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui
    @Environment(\.palette) private var p
    @Environment(\.accessibilityReduceMotion) private var reduce

    var body: some View {
        let isCurrent = article.id == store.player.articleId
        let collapsed = state == .c
        let restWidth = max(0, (state == .a ? fullWidth : fullWidth - 70) - 7 - 46 - 11 - 6)
        ZStack(alignment: .leading) {
            HStack(spacing: 11) {
                RingCover(title: article.title, progress: store.progressFraction(article))
                HStack(spacing: 4) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text(article.title).font(Fonts.ui(14, .bold)).lineLimit(1)
                        Text(subtitle(isCurrent)).font(Fonts.ui(12, .semibold)).foregroundStyle(p.accentText).lineLimit(1)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    Group {
                        if isCurrent && store.player.status == .buffering {
                            ProgressView().controlSize(.small).tint(p.t1)
                                .frame(width: 44, height: 44)
                                .accessibilityLabel("缓冲中")
                        } else {
                            Button { store.playOrToggle(article.id) } label: {
                                PlayGlyph(playing: isCurrent && store.isPlaying, size: 22)
                                    .frame(width: 44, height: 44).contentShape(Circle())
                            }
                            .buttonStyle(PressStyle())
                            .accessibilityLabel(isCurrent && store.isPlaying ? "暂停" : "播放")
                        }
                    }
                    Button { store.nextSentence() } label: {
                        IconView(.next, size: 22, weight: 1.9).frame(width: 44, height: 44).contentShape(Circle())
                    }
                    .buttonStyle(PressStyle())
                    .accessibilityLabel("下一句")
                    .disabled(!isCurrent)
                }
                .frame(width: restWidth)
                .opacity(collapsed ? 0 : 1)
                .allowsHitTesting(!collapsed)
                .animation(Motion.fade(reduce), value: collapsed)
            }
            .padding(.leading, 7)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .contentShape(Rectangle())
            .onTapGesture { store.isPlayerExpanded = true }
            .accessibilityElement(children: .contain)
            .accessibilityLabel("正在播放：\(article.title)")
            .accessibilityIdentifier("bar-player")

            if collapsed {
                // C：右侧小球，点按 → B
                Button { ui.expandPlayer() } label: {
                    Color.clear.frame(maxWidth: .infinity, maxHeight: .infinity).contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel("展开播放条")
                .accessibilityIdentifier("bar-player-ball")
            }
        }
    }

    private func subtitle(_ isCurrent: Bool) -> String {
        if isCurrent, store.player.status == .buffering, store.timeline?.sentences.first?.ready != true {
            return "正在转换…"
        }
        if isCurrent, let s = store.currentSentenceText { return s }
        let (i, n) = store.sentenceProgress(article)
        return "第 \(i)/\(n) 句 · 点按继续"
    }
}
