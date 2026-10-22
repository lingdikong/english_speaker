import SwiftUI

/// 线性图标（24 网格，SVG 路径与设计稿 `P` / `G` 对象一致，三端同一套）。
/// 用一个极简 SVG path 解析器把路径字符串转成 SwiftUI Path。
enum Icon: String {
    case home, lib, search, gear, plus, prev, next, loop, queue, down, close, annot, vol, textsize, sun, moon
    case wave, bookmark, cloud, more

    /// 描边元素（stroke）。
    var strokes: [SVGElement] {
        switch self {
        case .home: [.path("M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z")]
        case .lib: [.path("M5 4h4v16H5zM11 4h4v16h-4z"), .path("m17 5 3.5.8-3 14.6-3.5-.8")]
        case .search: [.circle(11, 11, 7), .path("m20 20-3.5-3.5")]
        case .plus: [.path("M12 5v14M5 12h14")]
        case .prev: [.path("M5 5v14"), .path("M19 6v12l-9-6z")]
        case .next: [.path("M19 5v14"), .path("M5 6v12l9-6z")]
        case .loop: [.path("M17 3l3 3-3 3"), .path("M4 11V9a3 3 0 0 1 3-3h13"), .path("M7 21l-3-3 3-3"),
                     .path("M20 13v2a3 3 0 0 1-3 3H4"), .path("M11.5 10.5 13 9.5v5")]
        case .queue: [.path("M4 6h11M4 12h11M4 18h7M18 14v6M15 17h6")]
        case .down: [.path("m6 9 6 6 6-6")]
        case .close: [.path("M6 6l12 12M18 6 6 18")]
        case .annot: [.path("M4 8c2.5 4 6 4 8 0s5.5-4 8 0")]
        case .vol: [.path("M11 5 6 9H3v6h3l5 4z"), .path("M15.5 8.5a5 5 0 0 1 0 7")]
        case .textsize: [.path("M4 18 9 6l5 12M5.8 14h6.4"), .path("M15 18l3-7 3 7M16 16h4")]
        case .sun: [.circle(12, 12, 4), .path("M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4")]
        case .moon: [.path("M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z")]
        case .gear: [.circle(12, 12, 3), .path("M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z")]
        case .wave: [.path("M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2")]
        case .bookmark: [.path("M6 3.5h12v17l-6-4-6 4z")]
        case .cloud: [.path("M7 18a4.5 4.5 0 0 1-.5-9 6 6 0 0 1 11.5 1.5A3.8 3.8 0 0 1 17.5 18z"), .path("M12 11v5M9.8 14l2.2 2 2.2-2")]
        case .more: []
        }
    }
}

enum SVGElement {
    case path(String)
    case circle(CGFloat, CGFloat, CGFloat)
    case rect(CGFloat, CGFloat, CGFloat, CGFloat, CGFloat)

    var path: Path {
        switch self {
        case let .path(d): SVGPath.parse(d)
        case let .circle(cx, cy, r): Path(ellipseIn: CGRect(x: cx - r, y: cy - r, width: 2 * r, height: 2 * r))
        case let .rect(x, y, w, h, rx): Path(roundedRect: CGRect(x: x, y: y, width: w, height: h), cornerRadius: rx)
        }
    }
}

/// 24×24 坐标系里的 Path，缩放到 frame。
struct SVGShape: Shape {
    let elements: [SVGElement]
    func path(in rect: CGRect) -> Path {
        var p = Path()
        for e in elements { p.addPath(e.path) }
        let s = min(rect.width, rect.height) / 24
        return p.applying(CGAffineTransform(scaleX: s, y: s)
            .concatenating(CGAffineTransform(translationX: rect.minX + (rect.width - 24 * s) / 2,
                                             y: rect.minY + (rect.height - 24 * s) / 2)))
    }
}

/// 线性图标：默认描边 1.75（选中 2.0），圆头圆角。
struct IconView: View {
    let icon: Icon
    var size: CGFloat = 22
    var weight: CGFloat = 1.75

    init(_ icon: Icon, size: CGFloat = 22, weight: CGFloat = 1.75) {
        self.icon = icon; self.size = size; self.weight = weight
    }

    var body: some View {
        Group {
            if icon == .more {
                MoreDots()
            } else {
                // stroke-width 以 24 网格计，换算到实际尺寸
                SVGShape(elements: icon.strokes)
                    .stroke(style: StrokeStyle(lineWidth: weight * size / 24, lineCap: .round, lineJoin: .round))
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

/// ⋯（实心三点）
struct MoreDots: View {
    var body: some View {
        SVGShape(elements: [.circle(5, 12, 1.8), .circle(12, 12, 1.8), .circle(19, 12, 1.8)]).fill()
    }
}

/// 实心播放 / 暂停（设计稿 PLAY / PAUSE）。
struct PlayGlyph: View {
    let playing: Bool
    var size: CGFloat
    var body: some View {
        Group {
            if playing {
                SVGShape(elements: [.rect(6, 5, 4, 14, 1.2), .rect(14, 5, 4, 14, 1.2)]).fill()
            } else {
                SVGShape(elements: [.path("M7 4.5v15l12-7.5z")]).fill()
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

/// 实心书签（已保存）。
struct BookmarkFill: View {
    var size: CGFloat = 14
    var body: some View {
        SVGShape(elements: [.path("M6 3.5h12v17l-6-4-6 4z")]).fill().frame(width: size, height: size)
    }
}

/// 品牌图形：圆角方块内 5 根竖条声波。
struct BrandMark: View {
    var size: CGFloat = 56
    @Environment(\.palette) private var p
    var body: some View {
        let k = size / 56
        HStack(spacing: 3 * k) {
            ForEach([14.0, 26, 36, 22, 12], id: \.self) { h in
                RoundedRectangle(cornerRadius: 2 * k).frame(width: 4 * k, height: h * k)
            }
        }
        .foregroundStyle(p.accentInk)
        .frame(width: size, height: size)
        .background(p.accent, in: RoundedRectangle(cornerRadius: 16 * k, style: .continuous))
        .accessibilityHidden(true)
    }
}

// MARK: - SVG path 解析（M L H V C S Q T A Z，含相对坐标与隐式重复）

enum SVGPath {
    static func parse(_ d: String) -> Path {
        var p = Path()
        var tokens = Tokenizer(d)
        var cmd: Character = "M"
        var cur = CGPoint.zero, start = CGPoint.zero
        var lastCtrl: CGPoint?
        while let next = tokens.peekCommand() ?? (tokens.hasNumber ? cmd : nil) {
            if tokens.peekCommand() != nil { tokens.skipCommand() }
            cmd = next
            let rel = cmd.isLowercase
            func pt() -> CGPoint {
                let x = tokens.number(), y = tokens.number()
                return rel ? CGPoint(x: cur.x + x, y: cur.y + y) : CGPoint(x: x, y: y)
            }
            switch cmd.uppercased().first! {
            case "M":
                cur = pt(); start = cur; p.move(to: cur)
                cmd = rel ? "l" : "L" // 后续坐标对是 lineto
                lastCtrl = nil
            case "L":
                cur = pt(); p.addLine(to: cur); lastCtrl = nil
            case "H":
                let x = tokens.number(); cur.x = rel ? cur.x + x : x; p.addLine(to: cur); lastCtrl = nil
            case "V":
                let y = tokens.number(); cur.y = rel ? cur.y + y : y; p.addLine(to: cur); lastCtrl = nil
            case "C":
                let c1 = pt(), c2 = pt(), e = pt()
                p.addCurve(to: e, control1: c1, control2: c2); cur = e; lastCtrl = c2
            case "S":
                let c1 = lastCtrl.map { CGPoint(x: 2 * cur.x - $0.x, y: 2 * cur.y - $0.y) } ?? cur
                let c2 = pt(), e = pt()
                p.addCurve(to: e, control1: c1, control2: c2); cur = e; lastCtrl = c2
            case "Q":
                let c = pt(), e = pt()
                p.addQuadCurve(to: e, control: c); cur = e; lastCtrl = c
            case "T":
                let c = lastCtrl.map { CGPoint(x: 2 * cur.x - $0.x, y: 2 * cur.y - $0.y) } ?? cur
                let e = pt(); p.addQuadCurve(to: e, control: c); cur = e; lastCtrl = c
            case "A":
                let rx = tokens.number(), ry = tokens.number(), rot = tokens.number()
                let large = tokens.flag(), sweep = tokens.flag()
                let e = pt()
                arc(&p, from: cur, to: e, rx: rx, ry: ry, rotation: rot, large: large, sweep: sweep)
                cur = e; lastCtrl = nil
            case "Z":
                p.closeSubpath(); cur = start; lastCtrl = nil
            default:
                return p
            }
        }
        return p
    }

    /// SVG 椭圆弧 → 三次贝塞尔（W3C 端点参数化转中心参数化）。
    private static func arc(_ p: inout Path, from p0: CGPoint, to p1: CGPoint, rx rx0: CGFloat, ry ry0: CGFloat,
                            rotation: CGFloat, large: Bool, sweep: Bool) {
        var rx = abs(rx0), ry = abs(ry0)
        if rx == 0 || ry == 0 || p0 == p1 { p.addLine(to: p1); return }
        let phi = rotation * .pi / 180
        let cosP = cos(phi), sinP = sin(phi)
        let dx = (p0.x - p1.x) / 2, dy = (p0.y - p1.y) / 2
        let x1 = cosP * dx + sinP * dy, y1 = -sinP * dx + cosP * dy
        let lam = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry)
        if lam > 1 { rx *= sqrt(lam); ry *= sqrt(lam) }
        let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1
        let den = rx * rx * y1 * y1 + ry * ry * x1 * x1
        var co = sqrt(max(0, num / den))
        if large == sweep { co = -co }
        let cxp = co * rx * y1 / ry, cyp = -co * ry * x1 / rx
        let cx = cosP * cxp - sinP * cyp + (p0.x + p1.x) / 2
        let cy = sinP * cxp + cosP * cyp + (p0.y + p1.y) / 2
        func ang(_ ux: CGFloat, _ uy: CGFloat, _ vx: CGFloat, _ vy: CGFloat) -> CGFloat {
            let a = atan2(ux * vy - uy * vx, ux * vx + uy * vy)
            return a
        }
        let t1 = ang(1, 0, (x1 - cxp) / rx, (y1 - cyp) / ry)
        var dt = ang((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry)
        if !sweep && dt > 0 { dt -= 2 * .pi } else if sweep && dt < 0 { dt += 2 * .pi }
        let segs = Int(ceil(abs(dt) / (.pi / 2)))
        let step = dt / CGFloat(segs)
        let k = 4.0 / 3.0 * tan(step / 4)
        var t = t1
        func point(_ t: CGFloat) -> CGPoint {
            CGPoint(x: cx + rx * cos(t) * cosP - ry * sin(t) * sinP, y: cy + rx * cos(t) * sinP + ry * sin(t) * cosP)
        }
        func deriv(_ t: CGFloat) -> CGPoint {
            CGPoint(x: -rx * sin(t) * cosP - ry * cos(t) * sinP, y: -rx * sin(t) * sinP + ry * cos(t) * cosP)
        }
        for _ in 0..<segs {
            let a = point(t), b = point(t + step), da = deriv(t), db = deriv(t + step)
            p.addCurve(to: b, control1: CGPoint(x: a.x + k * da.x, y: a.y + k * da.y),
                       control2: CGPoint(x: b.x - k * db.x, y: b.y - k * db.y))
            t += step
        }
    }

    private struct Tokenizer {
        let chars: [Character]
        var i = 0
        init(_ s: String) { chars = Array(s) }

        mutating func skipSeparators() {
            while i < chars.count, chars[i] == " " || chars[i] == "," || chars[i] == "\n" { i += 1 }
        }

        mutating func peekCommand() -> Character? {
            skipSeparators()
            guard i < chars.count, chars[i].isLetter, chars[i] != "e", chars[i] != "E" else { return nil }
            return chars[i]
        }

        mutating func skipCommand() { i += 1 }

        var hasNumber: Bool {
            mutating get {
                skipSeparators()
                guard i < chars.count else { return false }
                let c = chars[i]
                return c.isNumber || c == "-" || c == "." || c == "+"
            }
        }

        mutating func flag() -> Bool {
            skipSeparators()
            guard i < chars.count else { return false }
            let c = chars[i]; i += 1
            return c == "1"
        }

        /// 数字：支持 "-.5"、".8.3"（第二个点开始新数字）、"1e-3"。
        mutating func number() -> CGFloat {
            skipSeparators()
            var s = ""
            var seenDot = false, seenExp = false
            if i < chars.count, chars[i] == "-" || chars[i] == "+" { s.append(chars[i]); i += 1 }
            while i < chars.count {
                let c = chars[i]
                if c.isNumber { s.append(c) }
                else if c == "." && !seenDot && !seenExp { seenDot = true; s.append(c) }
                else if (c == "e" || c == "E") && !seenExp { seenExp = true; s.append(c)
                    if i + 1 < chars.count, chars[i + 1] == "-" || chars[i + 1] == "+" { i += 1; s.append(chars[i]) } }
                else { break }
                i += 1
            }
            return CGFloat(Double(s) ?? 0)
        }
    }
}
