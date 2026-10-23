import SpeakerCore
import SwiftUI

/// 播放页（UI_SPEC §4.5）：原文为中心，无大封面。
/// 顶部两行（收起 · 第 x/y 句 · 更多；封面 + 标题 + 音色 + 保存）；中间原文，当前句固定在约 35% 高度；
/// 底部玻璃控制面板。播放中 5 秒无操作进入专注模式。
struct PlayerView: View {
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui
    @Environment(Prefs.self) private var prefs
    @Environment(\.palette) private var p
    @Environment(\.accessibilityReduceMotion) private var reduce

    /// 收起进度（YouTube 式）：0 = 全屏，1 = 已收进底部播放条的小球。
    /// 由 RootView 持有：底层界面据此从 0.94 放大回 1。
    @Binding var collapse: CGFloat

    /// 迷你播放球直径：底部栏播放药丸是 60 高的胶囊，左端那个圆就是落点。
    static let ballSize: CGFloat = BottomBar.height
    /// 手指下拉多少 pt 走完 100% 进度（拉长一点，收起过程才不会“飞”）
    static let dismissDistance: CGFloat = 460

    @State private var focus = false
    @State private var lastInteraction = Date()
    @State private var wordSheet: WordRef?
    @State private var showReading = false
    @State private var showQueue = false
    /// 正在播放收起动画（避免手势重复触发）
    @State private var dismissing = false
    /// 已播过展开动画
    @State private var appeared = false
    /// 手势开始时冻结的高亮词：收起过程中歌词不再随播放推进重算，
    /// 否则逐词重排会和缩放变换叠在一起 → 闪烁（用户反馈的成因）。
    @State private var frozenWord: Int?

    /// 头部/控制面板等“外壳”随收起淡出，只留正文（和 YouTube 收起时一样）。
    private var chromeOpacity: Double { max(0, 1 - collapse * 1.8) }
    /// 完成收起时最后淡出，落到播放球上化掉（球一直就在下面，不会闪）。
    private var cardOpacity: Double { dismissing ? 0 : 1 }
    /// 收起进行中：冻结歌词的逐词高亮与自动滚动。
    private var frozen: Bool { collapse > 0 }

    /// 收起时页面的“取景窗”：从全屏连续变形到迷你播放球那个圆。
    private struct Morph {
        let width: CGFloat, height: CGFloat, radius: CGFloat
        let x: CGFloat, y: CGFloat, scale: CGFloat
    }

    /// 迷你播放球圆心（页面坐标）。几何与 BottomBar 一致：药丸左端的圆。
    private func ballCenter(width w: CGFloat, height h: CGFloat) -> (x: CGFloat, y: CGFloat) {
        let ball = Self.ballSize
        let bottom = ui.bar == .a ? BottomBar.bottom + BottomBar.height + BottomBar.gap : BottomBar.bottom
        let minX: CGFloat = switch ui.bar {
        case .a: BottomBar.side
        case .b: BottomBar.side + BottomBar.height + BottomBar.gap
        case .c: w - BottomBar.side - BottomBar.height
        }
        return (minX + ball / 2, h - bottom - ball / 2)
    }

    /// 全屏 → 迷你球：窗口宽高与圆心插值，页面按窗口宽度等比缩放。
    /// 宽度变成 60、高度也变成 60，窗口就是一个正方形；圆角长到半径 30 即正圆，
    /// 正好和底部播放球重合，最后在那里淡出 —— 放大就是同一条曲线反向。
    private func morph(width w: CGFloat, height h: CGFloat) -> Morph {
        // ① 必须夹到 0…1：弹簧回弹（展开 / 取消收起）会短暂冲过目标值，
        //    负值进度会算出比屏幕还大的窗口、圆角还是负的 → 收尾“跳”一下。
        let c = min(1, max(0, collapse))
        // ② 尺寸比位置慢一档（二次）：开头几乎只是跟着手往下走，末段才快速收成球。
        //    两者都线性时，卡片顶边要跑 766pt 而手指只走 340pt（2.25 倍）→ 感觉飞出去。
        let sp = c * c
        let b = Self.ballSize
        let bc = ballCenter(width: w, height: h)
        let cw = max(1, w + (b - w) * sp)
        return Morph(width: cw,
                     height: max(1, h + (b - h) * sp),
                     radius: (b / 2) * sp,
                     x: w / 2 + (bc.x - w / 2) * c,
                     y: h / 2 + (bc.y - h / 2) * c,
                     scale: max(0.001, cw / w))
    }

    var body: some View {
        // 外层保持原有语义（在安全区内），只用来换算「内容区相对屏幕顶部的位置」；
        // 内层铺满整屏。页面必须按**整屏**排版，否则：
        // ① 取景窗只有安全区那么高，页面上下会露出底层界面（背景 ignoresSafeArea 也被裁掉）；
        // ② 落点公式里的「屏高」少一个 bottom inset，小球位置会偏。
        GeometryReader { probe in
            let probeTop = probe.frame(in: .global).minY
            let contentTop = probeTop + (probe.safeAreaInsets.top > 0 ? 0 : 20)
            let fadeTop = probeTop + probe.safeAreaInsets.top + 60
            FullScreen { geo in
                let w = geo.size.width
                let h = geo.size.height
                let m = morph(width: w, height: h)
                ZStack(alignment: .topLeading) {
                    // 页面按全屏尺寸排版 → 整体缩放 → 放进逐渐变小的取景窗并裁切。
                    // 只对一个图层做变换，子层（mask、渐变、文字）不会再彼此错帧。
                    // 缩放锚定在**顶部**、窗口也顶部对齐：内容顶边始终贴着窗口顶边，
                    // header 不会被裁掉（收起手势就挂在 header 上，裁掉会中断手势），
                    // 被裁的是底部控制面板——和 YouTube 收起时面板滑出视野一致。
                    page(screenH: h, contentTop: contentTop, fadeTop: fadeTop, chrome: chromeOpacity)
                        .frame(width: w, height: h)
                        .scaleEffect(m.scale, anchor: .top)
                        .frame(width: m.width, height: m.height, alignment: .top)
                        .clipShape(RoundedRectangle(cornerRadius: m.radius, style: .continuous))
                        .position(x: m.x, y: m.y)
                        .opacity(cardOpacity)
                }
                .animation(reduce ? nil : .easeInOut(duration: 0.3), value: focus)
            }
        }
        .onAppear(perform: expand)
        .sheet(item: $wordSheet) { ref in WordSheet(ref: ref).themed() }
        .sheet(isPresented: $showReading) { ReadingSettingsSheet().themed() }
        .sheet(isPresented: $showQueue) { UpNextSheet().themed() }
        .task(id: focusKey) { await focusTimer() }
        .task { await debugHooks() }
        // 注意：不要在 body 级挂下滑退出的 DragGesture——它和原文 ScrollView 抢手势，
        // 专注模式下向下滑歌词会误触发退出。退出只走顶部跟手下滑（dismissDrag）与收起按钮。
    }

    /// 播放页本体（背景 + 原文 + 头部 + 控制面板 + 专注点按层）。
    /// contentTop：头部距**屏幕顶**的距离；fadeTop：原文顶部渐隐的高度；chrome：外壳不透明度。
    private func page(screenH: CGFloat, contentTop: CGFloat, fadeTop: CGFloat, chrome: Double) -> some View {
        ZStack(alignment: .top) {
            p.bg.ignoresSafeArea()

            TranscriptScroll(screenHeight: screenH, focus: focus,
                             frozen: frozen, frozenWord: frozenWord,
                             onTapWord: { s, w in
                interact()
                wordSheet = WordRef(sentence: s, word: w)
            }, onInteract: interact)
            // 顶部渐隐（专注模式下头部收起后，原文在状态栏下淡出）
            .mask {
                VStack(spacing: 0) {
                    LinearGradient(colors: [.clear, .black], startPoint: .top, endPoint: .bottom)
                        .frame(height: fadeTop)
                    Color.black
                }
                .ignoresSafeArea()
            }
            .ignoresSafeArea()

            header
                .padding(.top, contentTop)
                .background {
                    // 头部底色：原文从这里滚上去时被盖住，并向下 24pt 渐隐
                    p.bg.ignoresSafeArea(edges: .top)
                        .overlay(alignment: .bottom) {
                            LinearGradient(colors: [p.bg, p.bg.opacity(0)], startPoint: .top, endPoint: .bottom)
                                .frame(height: 24).offset(y: 24)
                        }
                }
                .opacity((focus ? 0 : 1) * chrome)
                // 只看 focus / dismissing：不能再按 chrome 阈值开关——收起过程中它会被反复
                // 切到 false，把手势打断，触发一次意外的 onEnded（页面弹回 → 闪烁）。
                .allowsHitTesting(!focus && !dismissing)
                .gesture(dismissDrag)

            VStack {
                Spacer()
                ControlPanel(focus: focus, showReading: $showReading, showQueue: $showQueue, onInteract: interact)
                    .padding(.horizontal, 10)
                    .padding(.bottom, 10)
            }
            .opacity(chrome)
            .ignoresSafeArea(edges: .bottom)

            if focus {
                // 专注模式：轻点任意处恢复（这次点按不触发其它操作）
                Color.clear.contentShape(Rectangle()).ignoresSafeArea()
                    .onTapGesture { interact() }
                    .accessibilityLabel("显示控件")
                    .accessibilityAddTraits(.isButton)
            }
        }
    }

    private var focusKey: String { "\(lastInteraction.timeIntervalSince1970)-\(store.isPlaying)-\(prefs.focusMode)" }

    private func interact() {
        lastInteraction = Date()
        if focus { focus = false }
    }

    /// 展开（进入播放页）：从小卡片位置放大到全屏——与收起是同一条曲线反向播放。
    private func expand() {
        guard !appeared else { return }
        appeared = true
        // 展开动画期间歌词也是冻结的（collapse > 0），先把高亮词定在当前位置
        frozenWord = store.word
        if collapse > 0 {
            // dampingFraction 1 = 临界阻尼，不冲过 0。
            // 之前用 0.88（欠阻尼）会在收尾冲成负进度 → 窗口比屏幕还大、圆角为负 → 展开到位时跳一下。
            withAnimation(reduce ? nil : .spring(response: 0.42, dampingFraction: 1)) { collapse = 0 }
        }
    }

    /// 收进底部播放条：走完剩余进度（缩小 → 落到播放条上 → 最后淡出），然后移除播放页。
    private func collapseToMini() {
        guard !dismissing else { return }
        // dismissing 必须和 collapse 放在同一个动画里：写在动画外的话，
        // cardOpacity 会瞬间跳到 0，页面是「啪」地消失而不是边缩边淡出（看着就是闪一下）。
        withAnimation(reduce ? nil : .easeInOut(duration: 0.26)) {
            dismissing = true
            collapse = 1
        }
        Task {
            try? await Task.sleep(for: .milliseconds(reduce ? 0 : 270))
            store.isPlayerExpanded = false
        }
    }

    /// 顶部下滑收起（YouTube 式）：进度 = 下拉距离 / dismissDistance，整页连续缩小并移向底部播放条；
    /// 松手按“走过一定距离或甩得够快”判定完成，否则弹回全屏。
    ///
    /// 必须用 `.global` 坐标系：手势挂在 header 上，而 header 正被 scaleEffect + offset 变换，
    /// 默认的 `.local` 会把手指位移按当前缩放值放大（页面越小，同一个动作算出的位移越大），
    /// 形成「位移↑ → 进度↑ → 缩放↓ → 位移↑」的正反馈，慢慢拖时进度会在多个解之间跳 → 闪烁。
    private var dismissDrag: some Gesture {
        DragGesture(minimumDistance: 10, coordinateSpace: .global)
            .onChanged { v in
                guard !dismissing else { return }
                // 手势刚开始：冻结当前高亮词，收起过程中歌词不再重算（见 frozenWord 说明）
                if collapse == 0 { frozenWord = store.word }
                // 取整到整数点：整点在 2x/3x 屏上对应整数像素，避免文字逐帧半像素重采样出现重影
                collapse = min(1, max(0, v.translation.height.rounded()) / Self.dismissDistance)
            }
            .onEnded { v in
                guard !dismissing else { return }
                let far = collapse > 0.3 || v.predictedEndTranslation.height > 320
                if far {
                    collapseToMini()
                } else {
                    // 临界阻尼：不冲过 0（负进度会让窗口超出屏幕、圆角为负）
                    withAnimation(reduce ? nil : .spring(response: 0.34, dampingFraction: 1)) { collapse = 0 }
                }
            }
    }

    /// 播放中 5 秒无操作 → 专注模式（设置里可关）。
    private func focusTimer() async {
        guard prefs.focusMode, store.isPlaying, !focus else { return }
        try? await Task.sleep(for: .seconds(5))
        if Task.isCancelled { return }
        if store.isPlaying && wordSheet == nil && !showReading && !showQueue { focus = true }
    }

    private func debugHooks() async {
        let d = UserDefaults.standard
        if d.bool(forKey: "focusNow") {
            try? await Task.sleep(for: .seconds(1.5))
            focus = true
        }
        if let n = d.string(forKey: "wordSheet").flatMap(Int.init) {
            try? await Task.sleep(for: .seconds(1.5))
            wordSheet = WordRef(sentence: store.sentence ?? 0, word: n)
        }
        switch d.string(forKey: "playerSheet") {
        case "reading": try? await Task.sleep(for: .seconds(1)); showReading = true
        case "queue": try? await Task.sleep(for: .seconds(1)); showQueue = true
        default: break
        }
    }

    // MARK: 顶部

    private var header: some View {
        let a = store.current
        let total = store.timeline?.sentences.count ?? Int(a?.totalSentences ?? 0)
        return VStack(spacing: 8) {
            HStack {
                IconButton(icon: .down, size: 24, weight: 1.9, label: "收起") { collapseToMini() }
                Spacer()
                Text(total > 0 ? "第 \((store.sentence ?? 0) + 1) / \(total) 句" : "")
                    .font(Fonts.ui(13, .bold)).foregroundStyle(p.t2)
                Spacer()
                Menu {
                    if let a { PlayerMenu(article: a, showReading: $showReading, showQueue: $showQueue) }
                } label: {
                    MoreDots().frame(width: 22, height: 22).frame(width: 44, height: 44).contentShape(Circle())
                }
                .accessibilityLabel("更多")
            }
            .padding(.horizontal, 12)
            if let a {
                HStack(spacing: 12) {
                    WaveCover(title: a.title, size: 48)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(a.title).font(Fonts.display(17)).lineLimit(1)
                        Text(store.voiceLabel(a)).font(Fonts.ui(13, .medium)).foregroundStyle(p.t2)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    Button { store.setPinned(a.id, !a.pinned) } label: {
                        Group {
                            if a.pinned { BookmarkFill(size: 18).foregroundStyle(p.accent) }
                            else { IconView(.bookmark, size: 18, weight: 1.9) }
                        }
                        .frame(width: 44, height: 44)
                        .background(p.control, in: Circle())
                    }
                    .buttonStyle(PressStyle())
                    .accessibilityLabel(a.pinned ? "已保存，点按取消保存" : "保存")
                }
                .padding(.horizontal, 20)
            }
        }
        .padding(.bottom, 4)
    }
}

/// 铺满整屏的几何容器：内部的 GeometryReader 读到的是**整屏**尺寸（忽略安全区）。
/// 直接把 GeometryReader 放在页面里读到的是安全区盒子（比屏幕矮一个状态栏 + 一个 home 条），
/// 页面就会比屏幕小一圈，落点公式也会整体偏掉。
private struct FullScreen<Content: View>: View {
    @ViewBuilder var content: (GeometryProxy) -> Content
    var body: some View {
        GeometryReader { geo in content(geo) }
            .ignoresSafeArea()
    }
}

struct WordRef: Identifiable, Hashable {
    let sentence: Int
    let word: Int
    var id: String { "\(sentence)-\(word)" }
}

/// 播放页 ⋯ 菜单。
private struct PlayerMenu: View {
    let article: ArticleSummary
    @Binding var showReading: Bool
    @Binding var showQueue: Bool
    @Environment(Store.self) private var store

    var body: some View {
        Button { store.setPinned(article.id, !article.pinned) } label: { Text(article.pinned ? "取消保存" : "保存（离线保留）") }
        Menu("换音色") {
            ForEach(store.voices, id: \.id) { v in
                Button { store.synthesize(article.id, voice: v.id) } label: {
                    if (article.voiceId ?? Store.defaultVoice) == v.id { Label(Fmt.voice(v), systemImage: "checkmark") }
                    else { Text(Fmt.voice(v)) }
                }
            }
        }
        Button { showReading = true } label: { Text("阅读设置") }
        Button { showQueue = true } label: { Text("待播清单") }
    }
}

// MARK: - 原文滚动区

private struct TranscriptScroll: View {
    let screenHeight: CGFloat
    let focus: Bool
    /// 收起手势进行中：冻结逐词高亮与自动滚动。
    /// 播放推进（store.word 变化）会让整个当前句卡片重排，叠加页面的缩放变换就表现为闪烁，
    /// 因此在收起过程中用 frozenWord 定住高亮，滚动也暂停。
    let frozen: Bool
    let frozenWord: Int?
    let onTapWord: (Int, Int) -> Void
    let onInteract: () -> Void
    @Environment(Store.self) private var store
    @Environment(Prefs.self) private var prefs
    @Environment(\.palette) private var p
    @Environment(\.accessibilityReduceMotion) private var reduce
    @State private var followPaused = false
    @State private var resumeTask: Task<Void, Never>?

    /// 当前句卡片顶部所在的屏幕高度比例（卡片中线约在 35%）。
    static let anchor: CGFloat = 0.33

    var body: some View {
        let sentences = store.timeline?.sentences ?? []
        let current = store.sentence ?? 0
        let liveWord = frozen ? frozenWord : store.word
        ScrollViewReader { proxy in
            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 22) {
                    Color.clear.frame(height: screenHeight * Self.anchor - 22)
                    ForEach(sentences, id: \.index) { s in
                        let i = Int(s.index)
                        Group {
                            if i == current && s.ready {
                                CurrentSentenceCard(sentence: s,
                                                    currentWord: store.player.status == .ended ? Int.max : liveWord,
                                                    annotations: prefs.showAnnotations,
                                                    showChips: !focus,
                                                    size: prefs.fontSize, lineHeight: prefs.currentLineHeight,
                                                    onTapWord: { w in onTapWord(i, w) })
                            } else {
                                PlainSentence(text: s.text, ready: s.ready, size: prefs.otherSize,
                                              lineHeight: prefs.otherLineHeight)
                                    .contentShape(Rectangle())
                                    .onTapGesture {
                                        onInteract()
                                        guard s.ready else { return }
                                        store.seekSentence(i)
                                        resumeFollow(proxy, to: i)
                                    }
                                    .accessibilityAddTraits(s.ready ? .isButton : [])
                                    .accessibilityHint(s.ready ? "跳到这一句" : "尚未转换")
                                    .accessibilityIdentifier("sentence-\(i)")
                            }
                        }
                        .padding(.top, i > 0 && sentences[i - 1].paragraph != s.paragraph ? 10 : 0)
                        .overlay(alignment: .top) { Color.clear.frame(height: 1).id(i) }
                    }
                    Color.clear.frame(height: screenHeight)
                }
                .padding(.horizontal, 22)
            }
            .fadeUnder(.player)
            .simultaneousGesture(DragGesture(minimumDistance: 10).onChanged { _ in userScrolled() })
            .onAppear {
                DispatchQueue.main.async { proxy.scrollTo(current, anchor: UnitPoint(x: 0, y: Self.anchor)) }
            }
            .onChange(of: current) { _, idx in
                // 收起过程中不自动滚动：滚动动画和页面缩放变换叠加会抖
                guard !followPaused, !frozen else { return }
                if reduce { proxy.scrollTo(idx, anchor: UnitPoint(x: 0, y: Self.anchor)) }
                else { withAnimation(Motion.scroll) { proxy.scrollTo(idx, anchor: UnitPoint(x: 0, y: Self.anchor)) } }
            }
            .onChange(of: sentences.count) { _, _ in
                guard !frozen else { return }
                proxy.scrollTo(current, anchor: UnitPoint(x: 0, y: Self.anchor))
            }
            .overlay(alignment: .bottom) {
                if followPaused {
                    Button { resumeFollow(proxy, to: current) } label: {
                        Chip("回到当前句", kind: .accent, height: 36, fontSize: 13, hPad: 14)
                            .glass(Capsule())
                    }
                    .buttonStyle(.plain)
                    .padding(.bottom, focus ? 140 : 290)
                    .transition(.opacity)
                }
            }
        }
    }

    private func userScrolled() {
        onInteract()
        if !followPaused { followPaused = true }
        resumeTask?.cancel()
        resumeTask = Task {
            try? await Task.sleep(for: .seconds(3))
            if !Task.isCancelled { followPaused = false }
        }
    }

    private func resumeFollow(_ proxy: ScrollViewProxy, to idx: Int) {
        resumeTask?.cancel()
        followPaused = false
        if reduce { proxy.scrollTo(idx, anchor: UnitPoint(x: 0, y: Self.anchor)) }
        else { withAnimation(Motion.scroll) { proxy.scrollTo(idx, anchor: UnitPoint(x: 0, y: Self.anchor)) } }
    }
}

// MARK: - 玻璃控制面板

private struct ControlPanel: View {
    let focus: Bool
    @Binding var showReading: Bool
    @Binding var showQueue: Bool
    let onInteract: () -> Void
    @Environment(Store.self) private var store
    @Environment(Prefs.self) private var prefs
    @Environment(\.palette) private var p

    var body: some View {
        let dur = Double(max(store.durationMs, 1))
        VStack(spacing: 14) {
            VStack(spacing: 6) {
                TimelineView(.periodic(from: .now, by: 0.1)) { ctx in
                    let pos = store.livePosition(at: ctx.date)
                    WaveProgress(key: store.current?.id ?? "player", played: min(1, pos / dur),
                                 buffered: min(1, Double(store.bufferedMs) / dur)) { f in
                        onInteract()
                        store.seek(to: UInt64(f * dur))
                    }
                }
                HStack {
                    Text(Fmt.time(Int64(store.positionMs)))
                    Spacer()
                    let remain = Int64(store.durationMs) - Int64(store.positionMs)
                    Text((store.player.durationFinal ? "" : "约 ") + "-" + Fmt.time(max(0, remain)))
                }
                .font(Fonts.ui(12, .bold)).monospacedDigit()
                .foregroundStyle(p.t2)
            }
            if focus {
                Text("专注模式 · 轻点任意处显示控件")
                    .font(Fonts.ui(12, .medium)).foregroundStyle(p.t2)
                    .padding(.top, 2).padding(.bottom, 6)
                    .transition(.opacity)
            } else {
                transport
                secondary.transition(.opacity)
            }
        }
        .padding(.top, 16)
        .padding(.horizontal, 18)
        .padding(.bottom, 20)
        .glass(RoundedRectangle(cornerRadius: 38, style: .continuous))
        .contentShape(RoundedRectangle(cornerRadius: 38, style: .continuous))
        .simultaneousGesture(TapGesture().onEnded { onInteract() })
    }

    private var transport: some View {
        HStack {
            Button { onInteract(); store.cycleRate() } label: {
                Chip(Fmt.rate(Double(store.player.rate)), kind: .neutral, height: 40, fontSize: 14, hPad: 14)
            }
            .buttonStyle(PressStyle())
            .accessibilityLabel("倍速 \(Fmt.rate(Double(store.player.rate)))")
            Spacer()
            IconButton(icon: .prev, size: 28, weight: 1.9, label: "上一句") { onInteract(); store.previousSentence() }
            Spacer()
            PlayButton(playing: store.isPlaying, diameter: 72, glyph: 28,
                       buffering: store.player.status == .buffering) {
                onInteract(); store.togglePlay()
            }
            .accessibilityIdentifier("player-play")
            Spacer()
            IconButton(icon: .next, size: 28, weight: 1.9, label: "下一句") { onInteract(); store.nextSentence() }
            Spacer()
            IconButton(icon: .loop, size: 24, color: store.loopSentence != nil ? p.accentText : p.t1,
                       label: store.loopSentence != nil ? "单句循环（已开启）" : "单句循环") {
                onInteract(); store.toggleLoop()
            }
        }
    }

    private var secondary: some View {
        HStack {
            Button { onInteract(); prefs.showAnnotations.toggle() } label: {
                Chip(kind: prefs.showAnnotations ? .violet : .muted, height: 36) {
                    IconView(.annot, size: 16, weight: 2)
                    Text("跟读标注")
                }
            }
            .buttonStyle(PressStyle())
            .accessibilityValue(prefs.showAnnotations ? "开" : "关")
            Spacer()
            Button { onInteract(); showReading = true } label: {
                Chip(kind: .muted, height: 36) {
                    IconView(.textsize, size: 16, weight: 1.9)
                    Text("阅读设置")
                }
            }
            .buttonStyle(PressStyle())
            Spacer()
            IconButton(icon: .queue, label: "待播清单") { onInteract(); showQueue = true }
        }
    }
}
