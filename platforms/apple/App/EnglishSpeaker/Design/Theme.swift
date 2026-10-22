import SpeakerCore
import SwiftUI
import UIKit

// MARK: - 颜色令牌（UI_SPEC §2.1，两套主题同结构）

extension Color {
    init(hex: UInt32, alpha: Double = 1) {
        self.init(.sRGB,
                  red: Double((hex >> 16) & 0xFF) / 255,
                  green: Double((hex >> 8) & 0xFF) / 255,
                  blue: Double(hex & 0xFF) / 255,
                  opacity: alpha)
    }
}

struct Shadow {
    let color: Color
    let radius: CGFloat
    let y: CGFloat
}

struct Palette {
    let isDark: Bool
    let bg, surface, card, control, line: Color
    let t1, t2, t3: Color
    let accent, accentInk, accentText, accentSoft: Color
    let violet, violetText, violetSoft: Color
    let waveB, waveE: Color
    let other, pending: Color
    let dangerText, dangerSoft: Color
    let glass, glassHi, glassEdge: Color
    let elev: Shadow
    let glassShadow: Shadow

    /// 原文字重：白天 700，夜间 650 → iOS 用 SemiBold 600（UI_SPEC §1.1-7）。
    var transcriptWeight: Fonts.Weight { isDark ? .semibold : .bold }

    static func dark(otherLevel: Int = 60) -> Palette {
        Palette(
            isDark: true,
            bg: Color(hex: 0x11141C), surface: Color(hex: 0x171B26), card: Color(hex: 0x1E2330),
            control: Color(hex: 0x282E3D), line: Color(hex: 0x262C3A),
            t1: Color(hex: 0xD8DCE5), t2: Color(hex: 0xA3AABA), t3: Color(hex: 0x868E9F),
            accent: Color(hex: 0x3CCFB9), accentInk: Color(hex: 0x0E1A18), accentText: Color(hex: 0x5FD9C6),
            accentSoft: Color(hex: 0x3CCFB9, alpha: 0.14),
            violet: Color(hex: 0xA196F2), violetText: Color(hex: 0xB7AEF6), violetSoft: Color(hex: 0xA196F2, alpha: 0.14),
            waveB: Color(hex: 0x4A5163), waveE: Color(hex: 0x262B37),
            other: Color(hex: 0xD8DCE5, alpha: 0.55 + Double(otherLevel - 60) / 100),
            pending: Color(hex: 0xD8DCE5, alpha: 0.28),
            dangerText: Color(hex: 0xF2A7A7), dangerSoft: Color(hex: 0xF2A7A7, alpha: 0.12),
            glass: Color(hex: 0x222734, alpha: 0.30), glassHi: Color(hex: 0xFFFFFF, alpha: 0.10),
            glassEdge: Color(hex: 0xFFFFFF, alpha: 0.07),
            elev: Shadow(color: .black.opacity(0.45), radius: 15, y: 10),
            glassShadow: Shadow(color: .black.opacity(0.50), radius: 22, y: 14))
    }

    static func light(otherLevel: Int = 60) -> Palette {
        let ink: UInt32 = 0x1E232E
        return Palette(
            isDark: false,
            bg: Color(hex: 0xF3F4F6), surface: Color(hex: 0xFAFAFB), card: Color(hex: 0xFFFFFF),
            control: Color(hex: 0xE7E9EE), line: Color(hex: 0xE2E5EA),
            t1: Color(hex: ink), t2: Color(hex: 0x555D6D), t3: Color(hex: 0x666E7E),
            accent: Color(hex: 0x0E9483), accentInk: Color(hex: 0xFFFFFF), accentText: Color(hex: 0x08786A),
            accentSoft: Color(hex: 0x0E9483, alpha: 0.12),
            violet: Color(hex: 0x6A57D6), violetText: Color(hex: 0x5A47C8), violetSoft: Color(hex: 0x6A57D6, alpha: 0.10),
            waveB: Color(hex: 0xBFC4CE), waveE: Color(hex: 0xE3E6EB),
            other: Color(hex: ink, alpha: 0.65 + Double(otherLevel - 60) / 100),
            pending: Color(hex: ink, alpha: 0.30),
            dangerText: Color(hex: 0xB42828), dangerSoft: Color(hex: 0xB42828, alpha: 0.08),
            glass: Color(hex: 0xFFFFFF, alpha: 0.34), glassHi: Color(hex: 0xFFFFFF, alpha: 0.95),
            glassEdge: Color(hex: ink, alpha: 0.07),
            elev: Shadow(color: Color(hex: ink, alpha: 0.12), radius: 15, y: 10),
            glassShadow: Shadow(color: Color(hex: ink, alpha: 0.16), radius: 22, y: 14))
    }
}

private struct PaletteKey: EnvironmentKey {
    static let defaultValue = Palette.dark()
}

extension EnvironmentValues {
    var palette: Palette {
        get { self[PaletteKey.self] }
        set { self[PaletteKey.self] = newValue }
    }
}

/// 根据当前 colorScheme 与“非当前句亮度”注入调色板。每个 Sheet 的根也要套一次。
struct Themed: ViewModifier {
    @Environment(\.colorScheme) private var scheme
    @Environment(Prefs.self) private var prefs

    func body(content: Content) -> some View {
        let p = scheme == .dark ? Palette.dark(otherLevel: prefs.otherLevel) : Palette.light(otherLevel: prefs.otherLevel)
        content
            .environment(\.palette, p)
            .tint(p.accent)
            .foregroundStyle(p.t1)
    }
}

extension View {
    func themed() -> some View { modifier(Themed()) }

    func shadow(_ s: Shadow) -> some View { shadow(color: s.color, radius: s.radius, y: s.y) }
}

// MARK: - 字体（UI_SPEC §2.2；随 Bundle 打包，Info.plist UIAppFonts 注册）

enum Fonts {
    enum Weight { case regular, medium, semibold, bold }

    /// 标题 / 品牌：Space Grotesk 600/700。
    static func display(_ size: CGFloat, _ w: Weight = .bold) -> Font {
        .custom(w == .bold ? "SpaceGrotesk-Bold" : "SpaceGrotesk-SemiBold", fixedSize: size)
    }

    /// 界面与原文：Manrope 500/600/700（中文由系统回退到 PingFang SC）。
    static func ui(_ size: CGFloat, _ w: Weight = .semibold) -> Font {
        .custom(manropeName(w), fixedSize: size)
    }

    static func uiFont(_ size: CGFloat, _ w: Weight) -> UIFont {
        UIFont(name: manropeName(w), size: size) ?? .systemFont(ofSize: size, weight: .semibold)
    }

    /// IPA 音标：Noto Sans 400/500。
    static func ipa(_ size: CGFloat, _ w: Weight = .regular) -> Font {
        .custom(w == .regular ? "NotoSans-Regular" : "NotoSans-Medium", fixedSize: size)
    }

    private static func manropeName(_ w: Weight) -> String {
        switch w {
        case .regular, .medium: "Manrope-Medium"
        case .semibold: "Manrope-SemiBold"
        case .bold: "Manrope-Bold"
        }
    }

    /// 让文本达到指定行高（CSS line-height）所需的 lineSpacing。
    static func lineSpacing(size: CGFloat, weight: Weight, lineHeight: CGFloat) -> CGFloat {
        max(0, lineHeight - uiFont(size, weight).lineHeight)
    }
}

/// 小标签：12/700，字距 0.08em。
struct LabelText: View {
    let text: String
    @Environment(\.palette) private var p
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text).font(Fonts.ui(12, .bold)).tracking(12 * 0.08).foregroundStyle(p.t2)
    }
}

/// 大标题 32 Space Grotesk，字距 -0.02em。
struct LargeTitle: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text).font(Fonts.display(32)).tracking(-0.64)
    }
}

// MARK: - 动效（UI_SPEC §2.3）

enum Motion {
    static let spring = Animation.spring(response: 0.42, dampingFraction: 0.86)
    static let fade = Animation.easeInOut(duration: 0.2)
    static let scroll = Animation.easeInOut(duration: 0.28)

    /// 减少动态效果时直接切换。
    static func spring(_ reduce: Bool) -> Animation? { reduce ? nil : spring }
    static func fade(_ reduce: Bool) -> Animation? { reduce ? nil : fade }
}

// MARK: - 客户端偏好（主题与阅读设置，UserDefaults）

enum ThemeMode: String, CaseIterable { case system, light, dark
    var scheme: ColorScheme? {
        switch self { case .system: nil; case .light: .light; case .dark: .dark }
    }
}

@MainActor
@Observable
final class Prefs {
    @ObservationIgnored private let d = UserDefaults.standard

    var theme: ThemeMode { didSet { d.set(theme.rawValue, forKey: "pref.theme") } }
    /// 当前句字号 20–30，标准 24；其它句 = 字号 − 5。
    var fontSize: Double { didSet { d.set(fontSize, forKey: "pref.fontSize") } }
    /// 行距：1.6 舒适 / 1.4 紧凑。
    var lineHeight: Double { didSet { d.set(lineHeight, forKey: "pref.lineHeight") } }
    /// 非当前句亮度 50 / 60 / 70。
    var otherLevel: Int { didSet { d.set(otherLevel, forKey: "pref.otherLevel") } }
    var focusMode: Bool { didSet { d.set(focusMode, forKey: "pref.focusMode") } }
    var showAnnotations: Bool { didSet { d.set(showAnnotations, forKey: "pref.annotations") } }
    var defaultVoice: String { didSet { d.set(defaultVoice, forKey: "pref.voice") } }
    var defaultRate: Double { didSet { d.set(defaultRate, forKey: "pref.rate") } }

    init() {
        // 启动参数 -theme light|dark 可覆盖（截图用）
        theme = ThemeMode(rawValue: d.string(forKey: "theme") ?? d.string(forKey: "pref.theme") ?? "") ?? .system
        fontSize = d.object(forKey: "pref.fontSize") as? Double ?? 24
        lineHeight = d.object(forKey: "pref.lineHeight") as? Double ?? 1.6
        otherLevel = d.object(forKey: "pref.otherLevel") as? Int ?? 60
        // d.bool 也能解析启动参数里的 "YES" / "NO"
        focusMode = d.object(forKey: "pref.focusMode") == nil ? true : d.bool(forKey: "pref.focusMode")
        showAnnotations = d.object(forKey: "pref.annotations") == nil ? true : d.bool(forKey: "pref.annotations")
        defaultVoice = d.string(forKey: "pref.voice") ?? Store.defaultVoice
        defaultRate = d.object(forKey: "pref.rate") as? Double ?? 1.0
    }

    var currentLineHeight: CGFloat { (fontSize * lineHeight).rounded() }
    var otherSize: CGFloat { fontSize - 5 }
    var otherLineHeight: CGFloat { (otherSize * lineHeight).rounded() }
}

// MARK: - 格式化

enum Fmt {
    /// 1:02 / 12:05 / 1:02:03
    static func time(_ ms: Int64) -> String {
        let total = max(0, Int(ms / 1000))
        let h = total / 3600, m = (total % 3600) / 60, s = total % 60
        return h > 0 ? String(format: "%d:%02d:%02d", h, m, s) : String(format: "%d:%02d", m, s)
    }

    /// “6 分钟”；估算值加“约”。
    static func minutes(_ ms: UInt64, estimated: Bool = false) -> String {
        let m = max(1, Int((Double(ms) / 60000).rounded()))
        return (estimated ? "约 " : "") + "\(m) 分钟"
    }

    static func rate(_ r: Double) -> String {
        let v = (r * 100).rounded() / 100
        if (v * 10) == (v * 10).rounded() { return String(format: "%.1f×", v) }
        return String(format: "%.2f×", v)
    }

    static func bytes(_ b: UInt64) -> String {
        ByteCountFormatter.string(fromByteCount: Int64(min(b, UInt64(Int64.max))), countStyle: .memory)
    }

    static func accent(_ a: Accent) -> String { a == .american ? "美音" : "英音" }

    /// “Heart · 美音”
    static func voice(_ v: Voice, separator: String = " · ") -> String { "\(v.name)\(separator)\(accent(v.accent))" }
}
