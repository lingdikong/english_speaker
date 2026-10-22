import SpeakerCore
import SwiftUI

/// 播放页（UI_SPEC §4.5）：原文为中心，无大封面。
/// 顶部两行（收起 · 第 x/y 句 · 更多；封面 + 标题 + 音色 + 保存）；中间原文，当前句固定在约 35% 高度；
/// 底部玻璃控制面板。播放中 5 秒无操作进入专注模式。
struct PlayerView: View {
    @Environment(Store.self) private var store
    @Environment(Prefs.self) private var prefs
    @Environment(\.palette) private var p
    @Environment(\.dismiss) private var dismiss
    @Environment(\.accessibilityReduceMotion) private var reduce

    @State private var focus = false
    @State private var lastInteraction = Date()
    @State private var wordSheet: WordRef?
    @State private var showReading = false
    @State private var showQueue = false

    var body: some View {
        GeometryReader { geo in
            let screenH = geo.size.height + geo.safeAreaInsets.top + geo.safeAreaInsets.bottom
            ZStack(alignment: .top) {
                p.bg.ignoresSafeArea()
                TranscriptScroll(screenHeight: screenH, focus: focus, onTapWord: { s, w in
                    interact()
                    wordSheet = WordRef(sentence: s, word: w)
                }, onInteract: interact)
                // 顶部渐隐（专注模式下头部收起后，原文在状态栏下淡出）
                .mask {
                    VStack(spacing: 0) {
                        LinearGradient(colors: [.clear, .black], startPoint: .top, endPoint: .bottom)
                            .frame(height: geo.safeAreaInsets.top + 60)
                        Color.black
                    }
                    .ignoresSafeArea()
                }
                .ignoresSafeArea()

                header
                    .padding(.top, geo.safeAreaInsets.top > 0 ? 0 : 20)
                    .background {
                        // 头部底色：原文从这里滚上去时被盖住，并向下 24pt 渐隐
                        p.bg.ignoresSafeArea(edges: .top)
                            .overlay(alignment: .bottom) {
                                LinearGradient(colors: [p.bg, p.bg.opacity(0)], startPoint: .top, endPoint: .bottom)
                                    .frame(height: 24).offset(y: 24)
                            }
                    }
                    .opacity(focus ? 0 : 1)
                    .allowsHitTesting(!focus)


                VStack {
                    Spacer()
                    ControlPanel(focus: focus, showReading: $showReading, showQueue: $showQueue, onInteract: interact)
                        .padding(.horizontal, 10)
                        .padding(.bottom, 10)
                }
                .ignoresSafeArea(edges: .bottom)

                if focus {
                    // 专注模式：轻点任意处恢复（这次点按不触发其它操作）
                    Color.clear.contentShape(Rectangle()).ignoresSafeArea()
                        .onTapGesture { interact() }
                        .accessibilityLabel("显示控件")
                        .accessibilityAddTraits(.isButton)
                }
            }
            .animation(reduce ? nil : .easeInOut(duration: 0.3), value: focus)
        }
        .sheet(item: $wordSheet) { ref in WordSheet(ref: ref).themed() }
        .sheet(isPresented: $showReading) { ReadingSettingsSheet().themed() }
        .sheet(isPresented: $showQueue) { UpNextSheet().themed() }
        .task(id: focusKey) { await focusTimer() }
        .task { await debugHooks() }
        .gesture(DragGesture(minimumDistance: 30).onEnded { v in
            if v.translation.height > 120 && abs(v.translation.width) < 80 { dismiss() }
        })
    }

    private var focusKey: String { "\(lastInteraction.timeIntervalSince1970)-\(store.isPlaying)-\(prefs.focusMode)" }

    private func interact() {
        lastInteraction = Date()
        if focus { focus = false }
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
                IconButton(icon: .down, size: 24, weight: 1.9, label: "收起") { dismiss() }
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
        ScrollViewReader { proxy in
            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 22) {
                    Color.clear.frame(height: screenHeight * Self.anchor - 22)
                    ForEach(sentences, id: \.index) { s in
                        let i = Int(s.index)
                        Group {
                            if i == current && s.ready {
                                CurrentSentenceCard(sentence: s,
                                                    currentWord: store.player.status == .ended ? Int.max : store.word,
                                                    annotations: prefs.showAnnotations && !focus,
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
                guard !followPaused else { return }
                if reduce { proxy.scrollTo(idx, anchor: UnitPoint(x: 0, y: Self.anchor)) }
                else { withAnimation(Motion.scroll) { proxy.scrollTo(idx, anchor: UnitPoint(x: 0, y: Self.anchor)) } }
            }
            .onChange(of: sentences.count) { _, _ in
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
