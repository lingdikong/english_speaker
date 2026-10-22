import SpeakerCore
import SwiftUI

/// 点词读音（UI_SPEC §4.5）：词组 34 Space Grotesk（‿ violet）+ IPA 20 violet-text + 发音按钮；
/// 类型 chip + 美式/英式；中文说明；“同类”示例卡；“从这里播放” / “0.75× 慢放这句”。
struct WordSheet: View {
    let ref: WordRef
    @Environment(Store.self) private var store
    @Environment(\.palette) private var p
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        let words = store.timeline?.sentences[safe: ref.sentence]?.words ?? []
        let info = WordInfo(words: words, index: ref.word)
        let american = store.current.flatMap { store.voice(of: $0) }?.accent != .british
        VStack(alignment: .leading, spacing: 18) {
            if let info {
                HStack(alignment: .top) {
                    VStack(alignment: .leading, spacing: 4) {
                        groupText(info).font(Fonts.display(34)).lineLimit(1).minimumScaleFactor(0.6)
                        if !info.ipa.isEmpty {
                            Text(info.ipa).font(Fonts.ipa(20)).foregroundStyle(p.violetText)
                        }
                    }
                    Spacer()
                    IconButton(icon: .vol, size: 20, weight: 1.9, fill: true, label: "播放这个词") {
                        store.playSpan(startMs: info.startMs, endMs: info.endMs)
                    }
                }
                HStack(spacing: 8) {
                    Chip(info.kind, kind: info.hasNote ? .violet : .muted)
                    Chip(american ? "美式发音" : "英式发音", kind: .muted)
                }
                info.explanation(p)
                    .font(Fonts.ui(15, .medium)).lineSpacing(25 - 20.5)
                    .foregroundStyle(p.t2)
                    .fixedSize(horizontal: false, vertical: true)
                if let similar = info.similar {
                    (Text("同类：").foregroundColor(p.t2) + similar(p))
                        .font(Fonts.ui(14, .medium)).lineSpacing(4)
                        .padding(.horizontal, 16).padding(.vertical, 14)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(p.card, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(p.line, lineWidth: 1))
                }
                HStack(spacing: 10) {
                    PrimaryButton(title: "从这里播放", height: 48, fontSize: 15) {
                        store.seek(to: info.startMs)
                        store.resume()
                        dismiss()
                    }
                    Button {
                        store.setRate(0.75)
                        store.seekSentence(ref.sentence)
                        store.resume()
                        dismiss()
                    } label: {
                        Text("0.75× 慢放这句").font(Fonts.ui(15, .bold)).foregroundStyle(p.t1)
                            .frame(maxWidth: .infinity).frame(height: 48)
                            .background(p.control, in: Capsule())
                    }
                    .buttonStyle(PressStyle())
                }
            } else {
                Text("没有这个词的读音信息").font(Fonts.ui(15)).foregroundStyle(p.t2)
            }
        }
        .padding(.horizontal, 22)
        .padding(.top, 28)
        .padding(.bottom, 12)
        .frame(maxHeight: .infinity, alignment: .top)
        .presentationDetents([.height(info == nil ? 200 : (info?.similar == nil ? 330 : 400))])
        .presentationDragIndicator(.visible)
        .presentationCornerRadius(28)
        .presentationBackground(p.surface)
    }

    private func groupText(_ info: WordInfo) -> Text {
        var t = Text("")
        for (i, w) in info.group.enumerated() {
            if i > 0 { t = t + Text(info.linked ? "‿" : " ").foregroundColor(p.violet) }
            t = t + Text(w).foregroundColor(p.t1)
        }
        return t
    }
}

/// 点词读音的内容（由时间轴里的 WordSpan.notes 推出，纯展示）。
struct WordInfo {
    let group: [String]
    let linked: Bool
    let ipa: String
    let kind: String
    let hasNote: Bool
    let startMs: UInt64
    let endMs: UInt64
    private let link: LinkKind?
    private let weak: String?
    private let flap: Bool
    private let elided: Bool

    init?(words: [WordSpan], index i: Int) {
        guard let w = words[safe: i] else { return nil }
        // 词组：本词与后一个连读词；或前一个词连读到本词
        var span = [i]
        if w.link != nil, i + 1 < words.count { span = [i, i + 1] }
        else if i > 0, words[i - 1].link != nil { span = [i - 1, i] }
        let ws = span.map { words[$0] }
        let first = ws[0]
        group = ws.map { $0.text.trimmingCharacters(in: .punctuationCharacters) }
        linked = ws.count > 1
        link = linked ? first.link : nil
        weak = w.weakForm
        flap = w.flap
        elided = w.elided
        let ipas = ws.map(\.ipa).filter { !$0.isEmpty }
        if !linked, let weak = w.weakForm { ipa = "/\(weak)/" }
        else { ipa = ipas.isEmpty ? "" : "/" + ipas.joined(separator: " ") + "/" }
        startMs = first.startMs
        endMs = ws.last!.endMs
        if let l = link { kind = l.title }
        else if weak != nil { kind = "弱读" }
        else if flap { kind = "闪音" }
        else if elided { kind = "省音" }
        else { kind = "常规读音" }
        hasNote = link != nil || weak != nil || flap || elided
    }

    func explanation(_ p: Palette) -> Text {
        func b(_ s: String) -> Text { Text(s).foregroundColor(p.t1).fontWeight(.bold) }
        func ipaT(_ s: String) -> Text { Text(s).font(Fonts.ipa(15)).foregroundColor(p.violetText) }
        let a = group.first ?? "", c = group.count > 1 ? group[1] : ""
        if let link {
            switch link {
            case .flap:
                return b(a) + Text(" 末尾的 t 夹在两个元音之间，读得像轻快的 d，并和后面的 ") + b(c) + Text(" 连成一个音节：") + ipaT(ipa) + Text("。")
            case .consonantVowel:
                return b(a) + Text(" 末尾的辅音直接接到 ") + b(c) + Text(" 开头的元音上，两个词连成一串读，中间不停顿：") + ipaT(ipa) + Text("。")
            case .glideJ:
                return b(a) + Text(" 以 /iː/、/aɪ/ 等元音结尾，后面的 ") + b(c) + Text(" 以元音开头，中间自然带出一个轻轻的 /j/。")
            case .glideW:
                return b(a) + Text(" 以 /uː/、/oʊ/ 等元音结尾，后面的 ") + b(c) + Text(" 以元音开头，中间自然带出一个轻轻的 /w/。")
            case .linkingR:
                return b(a) + Text(" 结尾的 r 平时在英式发音里不读，但后面的 ") + b(c) + Text(" 以元音开头，这个 r 要读出来，把两个词连起来。")
            case .sameSound:
                return b(a) + Text(" 的结尾和 ") + b(c) + Text(" 的开头是同一个辅音，只读一次，稍稍延长。")
            case .assimilation:
                return b(a) + Text(" 和 ") + b(c) + Text(" 相邻的音互相影响而改变，例如 d + y 读成 /dʒ/：") + ipaT(ipa) + Text("。")
            }
        }
        if let weak {
            return b(a) + Text(" 是虚词，在句中不重读，读成弱读形式 ") + ipaT("/\(weak)/") + Text("，元音弱化为 /ə/ 或省去。")
        }
        if flap { return b(a) + Text(" 中的 t 夹在元音之间，美式发音读成轻快的闪音 /ɾ/，像 water、better。") }
        if elided { return b(a) + Text(" 结尾的 t/d 后面紧跟辅音，常常不发出来，像 next day。") }
        return b(a) + Text(" 在这里按词典读音读，没有特别的连读或弱读。")
    }

    var similar: ((Palette) -> Text)? {
        let ex: [[String]]
        if let link {
            switch link {
            case .flap: ex = [["get", "it"], ["what", "are"], ["a lot", "of"]]
            case .consonantVowel: ex = [["pick", "it"], ["turn", "off"], ["an", "apple"]]
            case .glideJ: ex = [["see", "it"], ["my", "own"], ["the", "end"]]
            case .glideW: ex = [["go", "on"], ["do", "it"], ["two", "eggs"]]
            case .linkingR: ex = [["far", "away"], ["for", "ever"], ["more", "and"]]
            case .sameSound: ex = [["big", "game"], ["want", "to"], ["bad", "dog"]]
            case .assimilation: ex = [["did", "you"], ["would", "you"], ["meet", "you"]]
            }
        } else if weak != nil {
            ex = [["and"], ["of"], ["to"], ["can"]]
        } else if flap {
            ex = [["water"], ["better"], ["city"]]
        } else if elided {
            ex = [["next day"], ["last night"], ["just now"]]
        } else {
            return nil
        }
        return { p in
            var t = Text("")
            for (i, e) in ex.enumerated() {
                if i > 0 { t = t + Text(" · ").foregroundColor(p.t2) }
                for (j, w) in e.enumerated() {
                    if j > 0 { t = t + Text("‿").foregroundColor(p.violet).fontWeight(.bold) }
                    t = t + Text(w).foregroundColor(p.t1).fontWeight(.bold)
                }
            }
            return t
        }
    }
}

extension LinkKind {
    var title: String {
        switch self {
        case .consonantVowel: "辅音连元音"
        case .glideJ: "加 /j/ 过渡"
        case .glideW: "加 /w/ 过渡"
        case .linkingR: "连接 r"
        case .sameSound: "相同辅音合并"
        case .flap: "跨词闪音"
        case .assimilation: "同化"
        }
    }
}
