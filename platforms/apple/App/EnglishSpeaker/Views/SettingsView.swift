import SpeakerCore
import SwiftUI

/// 设置 Tab（UI_SPEC §4.7）：外观 / 阅读 / 朗读 / 本地缓存 / 关于。
struct SettingsView: View {
    @Environment(Store.self) private var store
    @Environment(Prefs.self) private var prefs
    @Environment(\.palette) private var p
    @State private var confirmClear = false
    @State private var clearedMessage: String?

    var body: some View {
        @Bindable var prefs = prefs
        TabScroll(tab: .settings) {
            VStack(alignment: .leading, spacing: 22) {
                LargeTitle("设置")
                section("外观") { ThemeSegmented() }
                section("阅读") {
                    VStack(alignment: .leading, spacing: 18) {
                        ReadingPreview()
                        FontSizeSlider()
                        LineHeightSegmented()
                        ReadingToggles()
                    }
                }
                section("朗读") {
                    GroupCard {
                        GroupRow(title: "默认音色") {
                            Menu {
                                ForEach(store.voices, id: \.id) { v in
                                    Button(Fmt.voice(v)) { prefs.defaultVoice = v.id }
                                }
                            } label: {
                                Chip(defaultVoiceLabel + " ›", kind: .neutral)
                            }
                        }
                        GroupRow(title: "默认倍速", divider: false) {
                            Menu {
                                ForEach(Store.rates, id: \.self) { r in
                                    Button(Fmt.rate(Double(r))) {
                                        prefs.defaultRate = Double(r)
                                        store.setRate(r)
                                    }
                                }
                            } label: {
                                Chip(Fmt.rate(prefs.defaultRate) + " ›", kind: .neutral)
                            }
                        }
                    }
                }
                section("本地缓存") { cacheCard }.id("cache")
                section("关于") { about }
            }
        }
        .confirmationDialog("清除未保存的缓存？", isPresented: $confirmClear, titleVisibility: .visible) {
            Button("清除", role: .destructive) {
                let n = store.clearCache()
                clearedMessage = "已清除 \(n) 篇文章的缓存"
            }
            Button("取消", role: .cancel) {}
        } message: {
            Text("已保存的文章和正在播放的文章不受影响。清除后再次播放时会重新转换。")
        }
    }

    private var defaultVoiceLabel: String {
        store.voices.first { $0.id == prefs.defaultVoice }.map { Fmt.voice($0) } ?? prefs.defaultVoice
    }

    private func section<C: View>(_ title: String, @ViewBuilder _ content: () -> C) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            LabelText(title)
            content()
        }
    }

    // MARK: 本地缓存

    private static let limits: [(UInt64, String)] = [
        (1 << 30, "1 GB"), (2 << 30, "2 GB"), (5 << 30, "5 GB"), (UInt64.max, "不限"),
    ]

    private var cacheCard: some View {
        let c = store.cache
        let used = Double(c?.usedBytes ?? 0)
        let pinned = Double(c?.pinnedBytes ?? 0)
        let limit = c?.limitBytes ?? (1 << 30)
        let unlimited = limit == UInt64.max
        // 不限时按已用量的 1.25 倍画条
        let scale = unlimited ? max(used * 1.25, 1) : Double(limit)
        return GroupCard {
            VStack(alignment: .leading, spacing: 10) {
                HStack(alignment: .firstTextBaseline) {
                    Text(Fmt.bytes(UInt64(used))).font(Fonts.display(22))
                    Text("/ " + (unlimited ? "不限" : Fmt.bytes(limit))).font(Fonts.ui(13, .semibold)).foregroundStyle(p.t2)
                    Spacer()
                    Text("\(c?.entries ?? 0) 篇").font(Fonts.ui(13, .semibold)).foregroundStyle(p.t2)
                }
                GeometryReader { g in
                    ZStack(alignment: .leading) {
                        Capsule().fill(p.waveE)
                        Capsule().fill(p.waveB).frame(width: max(0, g.size.width * min(1, used / scale)))
                        Capsule().fill(p.accent).frame(width: max(0, g.size.width * min(1, pinned / scale)))
                    }
                }
                .frame(height: 8)
                HStack(spacing: 14) {
                    legend(p.accent, "已保存 \(Fmt.bytes(UInt64(pinned)))")
                    legend(p.waveB, "可清理 \(Fmt.bytes(UInt64(max(0, used - pinned))))")
                }
            }
            .padding(.vertical, 12)
            .overlay(alignment: .bottom) { Rectangle().fill(p.line).frame(height: 1) }
            .accessibilityElement(children: .combine)
            GroupRow(title: "缓存上限") {
                Menu {
                    ForEach(Self.limits, id: \.0) { l in
                        Button(l.1) { store.setCacheLimit(l.0) }
                    }
                } label: {
                    Chip((Self.limits.first { $0.0 == limit }?.1 ?? Fmt.bytes(limit)) + " ›", kind: .neutral)
                }
            }
            Button { confirmClear = true } label: {
                HStack {
                    Text("清除未保存的缓存").font(Fonts.ui(15, .semibold)).foregroundStyle(p.dangerText)
                    Spacer()
                    if let clearedMessage { Text(clearedMessage).font(Fonts.ui(12)).foregroundStyle(p.t2) }
                }
                .frame(minHeight: 54)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .overlay(alignment: .bottom) { Rectangle().fill(p.line).frame(height: 1) }
            Text("已保存的文章不会被自动清理；超过上限时，最久没听的未保存文章会先被清理（原文保留）。")
                .font(Fonts.ui(12, .medium)).foregroundStyle(p.t2).lineSpacing(3)
                .padding(.vertical, 12)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func legend(_ c: Color, _ t: String) -> some View {
        HStack(spacing: 6) {
            Circle().fill(c).frame(width: 8, height: 8)
            Text(t).font(Fonts.ui(12, .semibold)).foregroundStyle(p.t2)
        }
    }

    // MARK: 关于

    private var about: some View {
        let version = Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "—"
        let build = Bundle.main.infoDictionary?["CFBundleVersion"] as? String ?? ""
        return GroupCard {
            GroupRow(title: "版本") { value("\(version) (\(build))") }
            GroupRow(title: "语音模型") { value(store.usingRealModel ? "Kokoro-82M v1.0 int8" : "Mock（未找到模型）") }
            GroupRow(title: "隐私") { value("完全离线，文章不会上传") }
            VStack(alignment: .leading, spacing: 6) {
                Text("开源许可").font(Fonts.ui(15, .semibold))
                Text("Kokoro-82M 模型 Apache-2.0 · ONNX Runtime MIT · misaki 词典 Apache-2.0 · Space Grotesk / Manrope / Noto Sans 字体 SIL OFL 1.1")
                    .font(Fonts.ui(12, .medium)).foregroundStyle(p.t2).lineSpacing(3)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.vertical, 14)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private func value(_ s: String) -> some View {
        Text(s).font(Fonts.ui(14, .medium)).foregroundStyle(p.t2).lineLimit(1).minimumScaleFactor(0.8)
    }
}

// MARK: - 阅读设置组件（设置页与播放页的“阅读设置”Sheet 共用同一组设置）

struct ThemeSegmented: View {
    @Environment(Prefs.self) private var prefs
    var body: some View {
        @Bindable var prefs = prefs
        Segmented(options: [(ThemeMode.system, "跟随系统", nil), (.light, "白天", .sun), (.dark, "夜间", .moon)],
                  selection: $prefs.theme)
    }
}

struct LineHeightSegmented: View {
    @Environment(Prefs.self) private var prefs
    var body: some View {
        @Bindable var prefs = prefs
        VStack(alignment: .leading, spacing: 10) {
            LabelText("行距")
            Segmented(options: [(1.6, "舒适 1.6", nil), (1.4, "紧凑 1.4", nil)], selection: $prefs.lineHeight)
        }
    }
}

struct ReadingPreview: View {
    @Environment(Prefs.self) private var prefs
    @Environment(\.palette) private var p
    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            LabelText("预览")
            AnnotatedSentence(words: SampleSentence.words, currentWord: 5, size: prefs.fontSize,
                              lineHeight: prefs.currentLineHeight, annotations: prefs.showAnnotations)
        }
        .padding(.horizontal, 18).padding(.vertical, 16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(p.card, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 20, style: .continuous).strokeBorder(p.line, lineWidth: 1))
    }
}

/// 字号滑杆 20–30（标准 24）：A ─●─ A
struct FontSizeSlider: View {
    @Environment(Prefs.self) private var prefs
    @Environment(\.palette) private var p

    var body: some View {
        let frac = (prefs.fontSize - 20) / 10
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                LabelText("字号")
                Spacer()
                Text("\(sizeName) · \(Int(prefs.fontSize))px").font(Fonts.ui(13, .medium)).foregroundStyle(p.t2)
            }
            HStack(spacing: 14) {
                Text("A").font(Fonts.ui(14, .bold))
                GeometryReader { g in
                    let w = g.size.width
                    ZStack(alignment: .leading) {
                        Capsule().fill(p.waveE).frame(height: 6)
                        Capsule().fill(p.accent).frame(width: max(6, w * frac), height: 6)
                        Circle().fill(p.card)
                            .overlay(Circle().strokeBorder(p.line, lineWidth: 1))
                            .shadow(color: .black.opacity(0.3), radius: 2, y: 1)
                            .frame(width: 24, height: 24)
                            .offset(x: w * frac - 12)
                    }
                    .frame(height: 24)
                    .frame(maxHeight: .infinity)
                    .contentShape(Rectangle())
                    .gesture(DragGesture(minimumDistance: 0).onChanged { v in
                        let f = max(0, min(1, v.location.x / w))
                        prefs.fontSize = (20 + f * 10).rounded()
                    })
                }
                .frame(height: 30)
                Text("A").font(Fonts.ui(22, .bold))
            }
        }
        .accessibilityElement()
        .accessibilityLabel("字号")
        .accessibilityValue("\(Int(prefs.fontSize))")
        .accessibilityAdjustableAction { dir in
            prefs.fontSize = max(20, min(30, prefs.fontSize + (dir == .increment ? 1 : -1)))
        }
    }

    private var sizeName: String {
        prefs.fontSize == 24 ? "标准" : (prefs.fontSize < 24 ? "较小" : "较大")
    }
}

struct ReadingToggles: View {
    @Environment(Prefs.self) private var prefs
    var body: some View {
        @Bindable var prefs = prefs
        GroupCard {
            GroupRow(title: "非当前句亮度") {
                Menu {
                    Button("较暗 50%") { prefs.otherLevel = 50 }
                    Button("适中 60%") { prefs.otherLevel = 60 }
                    Button("较亮 70%") { prefs.otherLevel = 70 }
                } label: {
                    Chip(levelName + " ›", kind: .neutral)
                }
            }
            GroupRow(title: "专注模式（5 秒后收起控件）") {
                AccentSwitch(isOn: $prefs.focusMode, label: "专注模式")
            }
            GroupRow(title: "显示跟读标注", divider: false) {
                AccentSwitch(isOn: $prefs.showAnnotations, label: "显示跟读标注")
            }
        }
    }

    private var levelName: String {
        switch prefs.otherLevel { case ..<55: "较暗 50%"; case 65...: "较亮 70%"; default: "适中 60%" }
    }
}

/// 播放页里打开的“阅读设置”Sheet（与设置页同一组设置）。
struct ReadingSettingsSheet: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var p

    var body: some View {
        ScrollView(showsIndicators: false) {
            VStack(alignment: .leading, spacing: 22) {
                SheetHeader(title: "阅读设置") { dismiss() }
                ReadingPreview()
                VStack(alignment: .leading, spacing: 10) {
                    LabelText("主题")
                    ThemeSegmented()
                }
                FontSizeSlider()
                LineHeightSegmented()
                ReadingToggles()
            }
            .padding(.horizontal, 20)
            .padding(.top, 20)
            .padding(.bottom, 30)
        }
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
        .presentationCornerRadius(28)
        .presentationBackground(p.bg)
    }
}
