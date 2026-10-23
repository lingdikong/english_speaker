import SpeakerCore
import SwiftUI
import UIKit

/// 原文与跟读标注（UI_SPEC §3.4）。
///
/// 当前句内：已读词 t1，当前词 accent-text + accent-soft 胶囊（圆角 9，左右 6），未读词 t2；
/// 连读 ‿ 用 violet 画在两词之间（替代空格）；弱读词 0.84em、t3（已读后 t2）、字重 600；
/// 闪音/同化/省音：词下 2px violet 点线。标注关闭后只保留逐词高亮。
struct AnnotatedSentence: View {
    let words: [WordSpan]
    /// 当前词序号；nil = 整句未开始（全部 t2）；Int.max = 整句已读。
    let currentWord: Int?
    var size: CGFloat = 24
    var lineHeight: CGFloat = 38
    var annotations = true
    var onTapWord: ((Int) -> Void)?
    @Environment(\.palette) private var p
    @Environment(\.accessibilityReduceMotion) private var reduce

    var body: some View {
        let w = p.transcriptWeight
        let font = Fonts.uiFont(size, w)
        let space = (" " as NSString).size(withAttributes: [.font: font]).width
        TokenFlow(lineHeight: lineHeight, baseline: Self.baseline(font: font, lineHeight: lineHeight)) {
            ForEach(words.indices, id: \.self) { i in
                let word = words[i]
                let linked = annotations && word.link != nil && i < words.count - 1
                HStack(alignment: .firstTextBaseline, spacing: 0) {
                    token(word, index: i, weight: w)
                    if linked { LinkMark(size: size, font: font) }
                }
                .layoutValue(key: TrailingGap.self, value: linked ? 0 : space)
            }
        }
        .animation(reduce ? nil : .easeInOut(duration: 0.2), value: currentWord)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(words.map(\.text).joined(separator: " "))
    }

    static func baseline(font: UIFont, lineHeight: CGFloat) -> CGFloat {
        let content = font.ascender - font.descender
        return (lineHeight - content) / 2 + font.ascender
    }

    @ViewBuilder
    private func token(_ word: WordSpan, index i: Int, weight: Fonts.Weight) -> some View {
        let state: Int = {
            guard let c = currentWord else { return 1 }
            return i < c ? -1 : (i == c ? 0 : 1)
        }()
        let weak = annotations && word.weakForm != nil
        let dotted = annotations && (word.flap || word.elided || word.link == .flap || word.link == .assimilation)
        let color: Color = switch state {
        case -1: weak ? p.t2 : p.t1
        case 0: p.accentText
        default: weak ? p.t3 : p.t2
        }
        Text(word.text)
            .font(Fonts.ui(weak ? (size * 0.84).rounded() : size, weak ? .semibold : weight))
            .tracking(size * 0.003)
            .foregroundStyle(color)
            .overlay(alignment: .bottom) {
                if dotted {
                    DottedLine()
                        .stroke(p.violet, style: StrokeStyle(lineWidth: 2, dash: [2, 2.5]))
                        .frame(height: 2)
                        .offset(y: 1)
                }
            }
            // 胶囊只作背景向外扩张，不改变占位尺寸——否则命中词会变宽，TokenFlow 重新换行导致整段跳动。
            .background {
                if state == 0 {
                    RoundedRectangle(cornerRadius: 9, style: .continuous)
                        .fill(p.accentSoft)
                        .padding(.horizontal, -6)
                        .padding(.vertical, -1)
                }
            }
            .contentShape(Rectangle())
            .onTapGesture { onTapWord?(i) }
    }
}

/// 连读弧线 ‿（按字体度量画在基线下方，宽约 0.5em）。
struct LinkMark: View {
    let size: CGFloat
    let font: UIFont
    @Environment(\.palette) private var p

    var body: some View {
        // 用一个透明字形占位以获得基线对齐，再在其上画弧
        Text("n").font(Fonts.ui(size, .bold)).hidden()
            .frame(width: size * 0.6)
            .overlay(alignment: .topLeading) {
                GeometryReader { g in
                    let top = font.ascender + size * 0.05
                    Path { path in
                        path.move(to: CGPoint(x: g.size.width * 0.08, y: top))
                        path.addQuadCurve(to: CGPoint(x: g.size.width * 0.92, y: top),
                                          control: CGPoint(x: g.size.width * 0.5, y: top + size * 0.42))
                    }
                    .stroke(p.violet, style: StrokeStyle(lineWidth: max(1.6, size * 0.085), lineCap: .round))
                }
            }
            .accessibilityHidden(true)
    }
}

struct DottedLine: Shape {
    func path(in r: CGRect) -> Path {
        var p = Path()
        p.move(to: CGPoint(x: r.minX, y: r.midY))
        p.addLine(to: CGPoint(x: r.maxX, y: r.midY))
        return p
    }
}

/// 每个词后面的间距（连读时为 0，‿ 已占位）。
struct TrailingGap: LayoutValueKey {
    static let defaultValue: CGFloat = 6
}

/// 固定行高的流式布局：每行基线位于行顶 + baseline，与 CSS line-height 一致。
struct TokenFlow: Layout {
    let lineHeight: CGFloat
    let baseline: CGFloat

    private struct Item { let index: Int; let size: CGSize; let base: CGFloat; let gap: CGFloat }

    private func lines(_ subviews: Subviews, width: CGFloat) -> [[(Item, CGFloat)]] {
        var out: [[(Item, CGFloat)]] = [[]]
        var x: CGFloat = 0
        for (i, v) in subviews.enumerated() {
            let size = v.sizeThatFits(.unspecified)
            let base = v.dimensions(in: .unspecified)[VerticalAlignment.firstTextBaseline]
            let item = Item(index: i, size: size, base: base, gap: v[TrailingGap.self])
            if x + size.width > width + 0.5, !out[out.count - 1].isEmpty {
                out.append([])
                x = 0
            }
            out[out.count - 1].append((item, x))
            x += size.width + item.gap
        }
        return out
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        let ls = lines(subviews, width: width)
        let maxW = ls.map { l in l.last.map { $0.1 + $0.0.size.width } ?? 0 }.max() ?? 0
        return CGSize(width: proposal.width ?? maxW, height: CGFloat(ls.count) * lineHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for (li, line) in lines(subviews, width: bounds.width).enumerated() {
            let top = bounds.minY + CGFloat(li) * lineHeight
            for (item, x) in line {
                subviews[item.index].place(at: CGPoint(x: bounds.minX + x, y: top + baseline - item.base),
                                           proposal: ProposedViewSize(item.size))
            }
        }
    }
}

/// 非当前句：other（55–65% 亮度）；未合成：pending 且不可点。字重 600。
struct PlainSentence: View {
    let text: String
    let ready: Bool
    var size: CGFloat = 19
    var lineHeight: CGFloat = 30
    @Environment(\.palette) private var p

    var body: some View {
        Text(text)
            .font(Fonts.ui(size, .semibold))
            .tracking(size * 0.003)
            .lineSpacing(Fonts.lineSpacing(size: size, weight: .semibold, lineHeight: lineHeight))
            .padding(.vertical, Fonts.lineSpacing(size: size, weight: .semibold, lineHeight: lineHeight) / 2)
            .foregroundStyle(ready ? p.other : p.pending)
            .frame(maxWidth: .infinity, alignment: .leading)
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// 当前句卡片：card 底，圆角 22，accent-soft 1px 内描边 + elev 投影；下方连读/弱读计数 chip。
struct CurrentSentenceCard: View {
    let sentence: SentenceSpan
    let currentWord: Int?
    var annotations: Bool
    var showChips = true
    var size: CGFloat = 24
    var lineHeight: CGFloat = 38
    var onTapWord: ((Int) -> Void)?
    @Environment(\.palette) private var p

    var body: some View {
        let links = sentence.words.dropLast().filter { $0.link != nil }.count
        let weaks = sentence.words.filter { $0.weakForm != nil }.count
        VStack(alignment: .leading, spacing: 12) {
            if sentence.words.isEmpty {
                Text(sentence.text).font(Fonts.ui(size, p.transcriptWeight))
                    .lineSpacing(Fonts.lineSpacing(size: size, weight: p.transcriptWeight, lineHeight: lineHeight))
            } else {
                AnnotatedSentence(words: sentence.words, currentWord: currentWord, size: size,
                                  lineHeight: lineHeight, annotations: annotations, onTapWord: onTapWord)
            }
            if annotations && showChips && (links > 0 || weaks > 0) {
                HStack(spacing: 8) {
                    if links > 0 { Chip("\(links) 处连读", kind: .violet, height: 26, fontSize: 12) }
                    if weaks > 0 { Chip("\(weaks) 个弱读", kind: .muted, height: 26, fontSize: 12) }
                }
            }
        }
        .padding(18)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            RoundedRectangle(cornerRadius: 22, style: .continuous).fill(p.card).shadow(p.elev)
        }
        .overlay(RoundedRectangle(cornerRadius: 22, style: .continuous).strokeBorder(p.accentSoft, lineWidth: 1))
    }
}

/// 阅读设置预览、空状态用的示例句（与设计稿 CUR 相同）。
enum SampleSentence {
    static let words: [WordSpan] = {
        let spec: [(String, LinkKind?, String?, Bool)] = [
            ("Doctors", nil, nil, false), ("say", nil, nil, false), ("that", nil, "ðət", false), ("a", nil, "ə", false),
            ("short", nil, nil, false), ("walk", .consonantVowel, nil, false), ("after", nil, nil, false),
            ("each", nil, nil, false), ("meal", .consonantVowel, nil, false), ("is", nil, nil, false),
            ("one", .consonantVowel, nil, false), ("of", nil, "əv", false), ("the", nil, "ðə", false),
            ("easiest", nil, nil, false), ("ways", nil, nil, false), ("to", nil, "tə", false), ("stay", nil, nil, false),
            ("healthy.", nil, nil, false),
        ]
        return spec.enumerated().map { i, s in
            WordSpan(text: s.0, charStart: 0, charEnd: 0, startMs: UInt64(i * 300), endMs: UInt64(i * 300 + 300),
                     ipa: "", link: s.1, weakForm: s.2, flap: s.3, elided: false)
        }
    }()

    static let sentence = SentenceSpan(index: 0, paragraph: 0, text: words.map(\.text).joined(separator: " "),
                                       ready: true, startMs: 0, endMs: 5400, words: words)
}
