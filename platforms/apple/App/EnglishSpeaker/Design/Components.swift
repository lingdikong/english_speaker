import SpeakerCore
import SwiftUI

// MARK: - 玻璃材质（UI_SPEC §2.4）

/// ultraThinMaterial 上叠 glass 色 + 顶部 1px 高光 + 1px 内描边 + 投影。不用系统 TabView / Liquid Glass 外观。
struct GlassBackground<S: InsettableShape>: ViewModifier {
    let shape: S
    @Environment(\.palette) private var p

    func body(content: Content) -> some View {
        content
            .background {
                ZStack {
                    shape.fill(.ultraThinMaterial)
                    shape.fill(p.glass)
                    // 顶部 1px 内高光：把描边向下错开 1px 再用形状裁切，只露出顶部
                    shape.inset(by: 0.5).stroke(p.glassHi, lineWidth: 1)
                        .mask(LinearGradient(stops: [.init(color: .black, location: 0), .init(color: .clear, location: 0.18)],
                                             startPoint: .top, endPoint: .bottom))
                    shape.strokeBorder(p.glassEdge, lineWidth: 1)
                }
                .compositingGroup()
                .shadow(p.glassShadow)
            }
    }
}

extension View {
    func glass<S: InsettableShape>(_ shape: S) -> some View { modifier(GlassBackground(shape: shape)) }
}

// MARK: - 内容在玻璃下渐隐（UI_SPEC §2.5：底部保留约 20%）

struct FadeStops {
    /// (距底部的距离, 不透明度)，从上到下。
    let stops: [(CGFloat, Double)]
    /// 列表（两个药丸）`.fade-m`
    static let twoRows = FadeStops(stops: [(240, 1), (156, 0.62), (26, 0.36), (0, 0.2)])
    /// 列表（一行）`.fade-m1`
    static let oneRow = FadeStops(stops: [(170, 1), (86, 0.62), (26, 0.36), (0, 0.2)])
    /// 播放页 `.fade-mp`
    static let player = FadeStops(stops: [(330, 1), (250, 0.6), (30, 0.3), (0, 0.18)])
}

struct FadeUnder: ViewModifier {
    let stops: FadeStops
    func body(content: Content) -> some View {
        content.mask {
            GeometryReader { geo in
                let h = max(geo.size.height, 1)
                LinearGradient(stops: [.init(color: .black, location: 0)] + stops.stops.map {
                    .init(color: .black.opacity($0.1), location: max(0, min(1, 1 - $0.0 / h)))
                }, startPoint: .top, endPoint: .bottom)
            }
            .ignoresSafeArea()
        }
    }
}

extension View {
    func fadeUnder(_ s: FadeStops) -> some View { modifier(FadeUnder(stops: s)) }
}

// MARK: - Chip

enum ChipKind { case accent, violet, neutral, muted, ghost, danger }

struct Chip<Label: View>: View {
    var kind: ChipKind
    var height: CGFloat = 30
    var fontSize: CGFloat = 13
    var hPad: CGFloat = 12
    @ViewBuilder var label: Label
    @Environment(\.palette) private var p

    var body: some View {
        HStack(spacing: 6) { label }
            .font(Fonts.ui(fontSize, .bold))
            .lineLimit(1)
            .padding(.horizontal, hPad)
            .frame(height: height)
            .foregroundStyle(fg)
            .background(bg, in: Capsule())
            .overlay { if kind == .ghost { Capsule().strokeBorder(p.line, lineWidth: 1) } }
    }

    private var fg: Color {
        switch kind {
        case .accent: p.accentText
        case .violet: p.violetText
        case .neutral: p.t1
        case .muted, .ghost: p.t2
        case .danger: p.dangerText
        }
    }

    private var bg: Color {
        switch kind {
        case .accent: p.accentSoft
        case .violet: p.violetSoft
        case .neutral, .muted: p.control
        case .ghost: .clear
        case .danger: p.dangerSoft
        }
    }
}

extension Chip where Label == Text {
    init(_ text: String, kind: ChipKind, height: CGFloat = 30, fontSize: CGFloat = 13, hPad: CGFloat = 12) {
        self.init(kind: kind, height: height, fontSize: fontSize, hPad: hPad) { Text(text) }
    }
}

/// 合成状态 chip（UI_SPEC §3.3，替代旧版下载图标）。
struct StatusChip: View {
    let article: ArticleSummary
    var fontSize: CGFloat = 12
    @Environment(Store.self) private var store
    @Environment(\.palette) private var p

    var body: some View {
        switch article.status {
        case .notCached:
            Button { store.synthesize(article.id) } label: {
                Chip("未转换", kind: .ghost, height: 22, fontSize: fontSize, hPad: 8)
            }
            .buttonStyle(.plain)
            .accessibilityHint("开始转换")
        case .queued:
            Chip("排队中", kind: .accent, height: 22, fontSize: fontSize, hPad: 8)
        case .synthesizing, .partial:
            Chip(kind: .accent, height: 22, fontSize: fontSize, hPad: 8) {
                MiniProgress(fraction: fraction, width: 22)
                Text("\(article.readySentences)/\(article.totalSentences)").monospacedDigit()
            }
            .accessibilityLabel("转换中 \(article.readySentences)/\(article.totalSentences)")
        case .ready:
            if article.pinned {
                HStack(spacing: 4) {
                    BookmarkFill(size: 12).foregroundStyle(p.accent)
                    Text("已保存").font(Fonts.ui(fontSize, .bold)).foregroundStyle(p.accentText)
                }
            }
        case .failed:
            Button { store.synthesize(article.id) } label: {
                Chip("失败 · 重试", kind: .danger, height: 22, fontSize: fontSize, hPad: 8)
            }
            .buttonStyle(.plain)
        }
    }

    private var fraction: Double {
        article.totalSentences == 0 ? 0 : Double(article.readySentences) / Double(article.totalSentences)
    }
}

struct MiniProgress: View {
    let fraction: Double
    var width: CGFloat = 28
    @Environment(\.palette) private var p
    var body: some View {
        ZStack(alignment: .leading) {
            Capsule().fill(p.waveE)
            Capsule().fill(p.accent).frame(width: max(3, width * fraction))
        }
        .frame(width: width, height: 3)
    }
}

// MARK: - 按钮

/// 圆形图标按钮（44 触控区）。fill = control 底。
struct IconButton: View {
    let icon: Icon
    var size: CGFloat = 22
    var weight: CGFloat = 1.75
    var fill = false
    var color: Color?
    let label: String
    let action: () -> Void
    @Environment(\.palette) private var p

    var body: some View {
        Button(action: action) {
            IconView(icon, size: size, weight: weight)
                .foregroundStyle(color ?? p.t1)
                .frame(width: 44, height: 44)
                .background(fill ? p.control : .clear, in: Circle())
                .contentShape(Circle())
        }
        .buttonStyle(PressStyle())
        .accessibilityLabel(label)
    }
}

/// 实心 accent 圆形播放键。
struct PlayButton: View {
    let playing: Bool
    var diameter: CGFloat = 48
    var glyph: CGFloat = 20
    var buffering = false
    let action: () -> Void
    @Environment(\.palette) private var p

    var body: some View {
        Button(action: action) {
            ZStack {
                if buffering {
                    ProgressView().tint(p.accentInk).controlSize(diameter > 60 ? .large : .regular)
                } else {
                    PlayGlyph(playing: playing, size: glyph)
                }
            }
            .foregroundStyle(p.accentInk)
            .frame(width: diameter, height: diameter)
            .background(p.accent, in: Circle())
            .contentShape(Circle())
        }
        .buttonStyle(PressStyle())
        .accessibilityLabel(buffering ? "缓冲中" : (playing ? "暂停" : "播放"))
    }
}

/// 主按钮：accent 底药丸。
struct PrimaryButton: View {
    let title: String
    var height: CGFloat = 54
    var fontSize: CGFloat = 16
    var enabled = true
    let action: () -> Void
    @Environment(\.palette) private var p

    var body: some View {
        Button(action: action) {
            Text(title).font(Fonts.ui(fontSize, .bold))
                .foregroundStyle(p.accentInk)
                .frame(maxWidth: .infinity).frame(height: height)
                .background(p.accent.opacity(enabled ? 1 : 0.45), in: Capsule())
                .contentShape(Capsule())
        }
        .buttonStyle(PressStyle())
        .disabled(!enabled)
    }
}

struct PressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .opacity(configuration.isPressed ? 0.7 : 1)
            .animation(.easeOut(duration: 0.15), value: configuration.isPressed)
    }
}

// MARK: - 开关（50×30，accent 底 + accent-ink 圆点）

struct AccentSwitch: View {
    @Binding var isOn: Bool
    let label: String
    @Environment(\.palette) private var p
    @Environment(\.accessibilityReduceMotion) private var reduce

    var body: some View {
        Button {
            withAnimation(Motion.spring(reduce)) { isOn.toggle() }
        } label: {
            ZStack(alignment: isOn ? .trailing : .leading) {
                Capsule().fill(isOn ? p.accent : p.control)
                Circle().fill(isOn ? p.accentInk : p.card)
                    .frame(width: 24, height: 24)
                    .shadow(color: .black.opacity(isOn ? 0 : 0.18), radius: 1.5, y: 1)
                    .padding(3)
            }
            .frame(width: 50, height: 30)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
        .accessibilityValue(isOn ? "开" : "关")
        .accessibilityAddTraits(.isButton)
    }
}

// MARK: - 分段控件

struct Segmented<T: Hashable>: View {
    let options: [(T, String, Icon?)]
    @Binding var selection: T
    /// 导入页风格：bg 底 + 选中 card；设置页风格：surface 底 + 选中 control。
    var importStyle = false
    var height: CGFloat = 40
    @Environment(\.palette) private var p
    @Environment(\.accessibilityReduceMotion) private var reduce

    var body: some View {
        HStack(spacing: 0) {
            ForEach(options.indices, id: \.self) { i in
                let (value, title, icon) = options[i]
                let on = value == selection
                Button {
                    withAnimation(Motion.fade(reduce)) { selection = value }
                } label: {
                    HStack(spacing: 6) {
                        if let icon { IconView(icon, size: 16) }
                        Text(title)
                    }
                    .font(Fonts.ui(14, .bold))
                    .foregroundStyle(on ? p.t1 : p.t2)
                    .frame(maxWidth: .infinity).frame(height: height)
                    .background {
                        if on {
                            RoundedRectangle(cornerRadius: 11, style: .continuous)
                                .fill(importStyle ? p.card : p.control)
                                .shadow(color: .black.opacity(importStyle ? 0.08 : 0), radius: 1, y: 1)
                        }
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityAddTraits(on ? .isSelected : [])
            }
        }
        .padding(4)
        .background(importStyle ? p.bg : p.surface, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay {
            if !importStyle { RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(p.line, lineWidth: 1) }
        }
    }
}

// MARK: - 分组卡片（card 底，行高 54，行间 line 分隔）

struct GroupCard<Content: View>: View {
    var hPad: CGFloat = 16
    @ViewBuilder var content: Content
    @Environment(\.palette) private var p

    var body: some View {
        VStack(spacing: 0) { content }
            .padding(.horizontal, hPad)
            .padding(.vertical, 4)
            .background(p.card, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(p.line, lineWidth: 1))
    }
}

struct GroupRow<Trailing: View>: View {
    let title: String
    var divider = true
    var height: CGFloat = 54
    @ViewBuilder var trailing: Trailing
    @Environment(\.palette) private var p

    var body: some View {
        HStack(spacing: 12) {
            Text(title).font(Fonts.ui(15, .semibold)).foregroundStyle(p.t1)
            Spacer(minLength: 8)
            trailing
        }
        .frame(minHeight: height)
        .overlay(alignment: .bottom) { if divider { Rectangle().fill(p.line).frame(height: 1) } }
    }
}

/// 面板（surface + line 内描边）。
struct Panel<Content: View>: View {
    var radius: CGFloat = 22
    var padding: CGFloat = 16
    @ViewBuilder var content: Content
    @Environment(\.palette) private var p

    var body: some View {
        content
            .padding(padding)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(p.surface, in: RoundedRectangle(cornerRadius: radius, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).strokeBorder(p.line, lineWidth: 1))
    }
}

/// Sheet 顶部拖拽条。
struct Grabber: View {
    @Environment(\.palette) private var p
    var body: some View {
        RoundedRectangle(cornerRadius: 3).fill(p.control).frame(width: 40, height: 5)
            .frame(maxWidth: .infinity)
    }
}

/// Sheet 标题行：关闭 · 标题 · 占位。
struct SheetHeader: View {
    let title: String
    let onClose: () -> Void
    var body: some View {
        HStack {
            IconButton(icon: .close, size: 20, weight: 2, fill: true, label: "关闭", action: onClose)
            Spacer()
            Text(title).font(Fonts.display(20))
            Spacer()
            Color.clear.frame(width: 44, height: 44)
        }
    }
}
