import SpeakerCore
import SwiftUI

// 翻译与查词（核心层双模：Fast = Opus-MT + ECDICT，Smart = 本地 Qwen）。
// 平台层只负责展示：译文、词典释义、举一反三、句子解析、生词本、术语表、模型下载。

// MARK: - 原文里的译文行

/// 句子下方的中文译文（双语模式）。比原文小一号，跟随整句明暗。
struct TranslationLine: View {
    let text: String?
    var current = false
    @Environment(Prefs.self) private var prefs
    @Environment(\.palette) private var p

    var body: some View {
        let size = (prefs.otherSize * 0.82).rounded()
        Text(text ?? (current ? "翻译中…" : " "))
            .font(Fonts.ui(size, .medium))
            .lineSpacing(size * 0.5)
            .foregroundStyle(current ? p.t2 : p.other)
            .opacity(text == nil ? 0.5 : 1)
            .frame(maxWidth: .infinity, alignment: .leading)
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// 当前句下方：“解析这句”按钮 / 解析结果（智能模式）。
struct SentenceAnalysisPanel: View {
    let sentence: Int
    @Environment(Store.self) private var store
    @Environment(\.palette) private var p
    @State private var open = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if !open {
                Button {
                    open = true
                    store.requestAnalysis(sentence: sentence)
                } label: {
                    Chip(kind: .violet, height: 30) {
                        IconView(.sparkle, size: 14, weight: 2)
                        Text("解析这句")
                    }
                }
                .buttonStyle(PressStyle())
            } else if let a = store.analyses[sentence]?.value {
                Text(a.translation).font(Fonts.ui(15, .semibold)).foregroundStyle(p.t1)
                if !a.structure.isEmpty {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Chip("结构", kind: .violet, height: 24, fontSize: 11)
                        Text(a.structure)
                    }
                }
                ForEach(a.points.indices, id: \.self) { i in
                    let pt = a.points[i]
                    (Text(pt.text + "  ").foregroundColor(p.violetText).fontWeight(.bold) + Text(pt.note))
                }
            } else if let e = store.analyses[sentence]?.error {
                Text("解析失败：\(e)")
            } else {
                HStack(spacing: 8) { ProgressView(); Text("正在用本地模型解析这句…") }
            }
        }
        .font(Fonts.ui(14, .medium))
        .foregroundStyle(p.t2)
        .lineSpacing(4)
        .fixedSize(horizontal: false, vertical: true)
        .padding(open ? 16 : 0)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            if open { RoundedRectangle(cornerRadius: 18, style: .continuous).fill(p.card) }
        }
        .overlay {
            if open { RoundedRectangle(cornerRadius: 18, style: .continuous).strokeBorder(p.line, lineWidth: 1) }
        }
        .onChange(of: sentence) { _, _ in open = false }
    }
}

// MARK: - 点词面板里的词典 / 举一反三

/// 离线词典释义：变形说明 · 前 3 条释义 · 考纲标签。
struct DictionarySection: View {
    let entry: DictEntry
    @Environment(\.palette) private var p

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let infl = entry.inflection {
                (Text(entry.query).bold().foregroundColor(p.t1) + Text(" 是 ") + Text(entry.word).bold().foregroundColor(p.t1)
                    + Text(" 的\(infl)"))
                    .font(Fonts.ui(13, .medium)).foregroundStyle(p.t2)
            }
            ForEach(entry.senses.prefix(3).indices, id: \.self) { i in
                let s = entry.senses[i]
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    if !s.pos.isEmpty { Text(s.pos).italic().font(Fonts.ui(13, .medium)).foregroundStyle(p.t2) }
                    Text(s.meaning).font(Fonts.ui(15, .medium)).foregroundStyle(p.t1)
                }
            }
            if !entry.tags.isEmpty {
                HStack(spacing: 6) {
                    ForEach(entry.tags, id: \.self) { Chip($0, kind: .ghost, height: 22, fontSize: 11, hPad: 8) }
                }
                .padding(.top, 2)
            }
        }
        .fixedSize(horizontal: false, vertical: true)
    }
}

/// 举一反三：语境义 · 常用搭配 · 地道例句（智能模式，后台生成后经事件送达）。
struct InsightSection: View {
    let sentence: Int
    let word: Int
    @Environment(Store.self) private var store
    @Environment(\.palette) private var p

    var body: some View {
        let r = store.insights["\(sentence):\(word)"]
        if let v = r?.value {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    Chip("语境义", kind: .violet, height: 24, fontSize: 11)
                    if !v.pos.isEmpty { Text(v.pos).italic().font(Fonts.ui(13, .medium)).foregroundStyle(p.t2) }
                    Text(v.meaning).font(Fonts.ui(15, .bold)).foregroundStyle(p.t1)
                }
                if !v.collocations.isEmpty {
                    Text(v.collocations.joined(separator: "  ·  "))
                        .font(Fonts.ui(14, .bold)).foregroundStyle(p.violetText)
                }
                ForEach(v.examples.indices, id: \.self) { i in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(v.examples[i].en).foregroundStyle(p.t1)
                        Text(v.examples[i].zh).foregroundStyle(p.t2)
                    }
                    .font(Fonts.ui(13, .medium))
                }
            }
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 16).padding(.vertical, 14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(p.card, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(p.line, lineWidth: 1))
        } else if r?.loading == true {
            HStack(spacing: 8) {
                ProgressView()
                Text("正在结合这句话分析…").font(Fonts.ui(14, .medium)).foregroundStyle(p.t2)
            }
        } else {
            Button { store.requestInsight(sentence: sentence, word: word) } label: {
                Chip(kind: .violet, height: 32) {
                    IconView(.sparkle, size: 14, weight: 2)
                    Text(r?.error == nil ? "举一反三" : "举一反三（重试）")
                }
            }
            .buttonStyle(PressStyle())
        }
    }
}

// MARK: - 设置页“翻译”组

struct TranslationSettings: View {
    @Environment(Store.self) private var store
    @Environment(\.palette) private var p
    @State private var confirmDownload = false
    @State private var confirmDelete = false
    @State private var showVocab = false
    @State private var showGlossary = false

    var body: some View {
        @Bindable var store = store
        if let t = store.translator {
            VStack(alignment: .leading, spacing: 10) {
                GroupCard {
                    GroupRow(title: "双语显示") {
                        AccentSwitch(isOn: $store.bilingual, label: "双语显示")
                    }
                    GroupRow(title: "智能翻译") {
                        AccentSwitch(isOn: Binding(get: { t.smartEnabled }, set: { on in
                            if on && t.llm.state != .downloaded && t.llm.state != .downloading {
                                confirmDownload = true
                            } else {
                                store.setSmart(on)
                            }
                        }), label: "智能翻译")
                    }
                    modelRow(t.llm)
                    GroupRow(title: "术语表") {
                        Button { store.reloadGlossary(); showGlossary = true } label: {
                            Chip("\(store.glossary.count) 条 ›", kind: .neutral)
                        }
                        .buttonStyle(.plain)
                    }
                    GroupRow(title: "生词本", divider: false) {
                        Button { showVocab = true } label: { Chip("\(store.vocab.count) 个词 ›", kind: .neutral) }
                            .buttonStyle(.plain)
                    }
                }
                Text(note(t))
                    .font(Fonts.ui(12, .medium)).foregroundStyle(p.t2).lineSpacing(3)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .onAppear { store.reloadGlossary() }
            .confirmationDialog("下载智能翻译模型？", isPresented: $confirmDownload, titleVisibility: .visible) {
                Button("下载并开启") {
                    store.setSmart(true)
                    store.downloadLlm()
                }
                Button("取消", role: .cancel) {}
            } message: {
                Text("需要下载 \(t.llm.modelName)（约 \(Fmt.bytes(t.llm.totalBytes))）"
                    + (store.onCellular ? "，当前正在使用蜂窝网络，建议连接 Wi-Fi 后下载" : "")
                    + "。下载后完全离线运行，约占用 500 MB 内存，可随时删除。")
            }
            .confirmationDialog("删除智能翻译模型？", isPresented: $confirmDelete, titleVisibility: .visible) {
                Button("删除", role: .destructive) { store.deleteLlm() }
                Button("取消", role: .cancel) {}
            } message: {
                Text("删除后整句翻译退回极速模式，已生成的译文与解析会保留。")
            }
            .sheet(isPresented: $showVocab) { VocabSheet().themed() }
            .sheet(isPresented: $showGlossary) { GlossarySheet().themed() }
        }
    }

    private func note(_ t: TranslatorStatus) -> String {
        var s = t.active == .smart ? "整句翻译：智能翻译（以极速译文为参考精修）。"
            : t.active == .fast ? "整句翻译：极速翻译（Opus-MT，内置）。" : ""
        if t.smartEnabled && t.active != .smart { s += "智能翻译模型下载完成后自动启用。" }
        return s
    }

    @ViewBuilder
    private func modelRow(_ llm: LlmStatus) -> some View {
        let pct = llm.totalBytes > 0 ? Double(llm.downloadedBytes) / Double(llm.totalBytes) : 0
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    Text(llm.modelName).font(Fonts.ui(14, .semibold)).foregroundStyle(p.t1)
                    if llm.loaded { Chip("运行中", kind: .accent, height: 20, fontSize: 10, hPad: 7) }
                }
                switch llm.state {
                case .downloading:
                    ProgressView(value: pct).tint(p.accent)
                    Text("正在下载 \(Fmt.bytes(llm.downloadedBytes)) / \(Fmt.bytes(llm.totalBytes))")
                        .font(Fonts.ui(12, .medium)).foregroundStyle(p.t2)
                case .downloaded:
                    Text("已下载 · \(Fmt.bytes(llm.totalBytes)) · 完全离线运行").font(Fonts.ui(12, .medium)).foregroundStyle(p.t2)
                case .failed:
                    Text("下载失败：\(llm.error ?? "")").font(Fonts.ui(12, .medium)).foregroundStyle(p.dangerText)
                case .notDownloaded:
                    Text("未下载 · 约 \(Fmt.bytes(llm.totalBytes))"
                        + (llm.downloadedBytes > 0 ? "（已下载 \(Fmt.bytes(llm.downloadedBytes))，可继续）" : ""))
                        .font(Fonts.ui(12, .medium)).foregroundStyle(p.t2)
                }
            }
            Spacer(minLength: 8)
            switch llm.state {
            case .downloading:
                Button { store.cancelLlmDownload() } label: { Chip("暂停", kind: .neutral) }.buttonStyle(.plain)
            case .downloaded:
                Button { confirmDelete = true } label: { Chip("删除", kind: .danger) }.buttonStyle(.plain)
            case .failed, .notDownloaded:
                Button { confirmDownload = true } label: {
                    Chip(llm.downloadedBytes > 0 || llm.state == .failed ? "继续下载" : "下载", kind: .accent)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.vertical, 12)
        .overlay(alignment: .bottom) { Rectangle().fill(p.line).frame(height: 1) }
    }
}

// MARK: - 生词本

struct VocabSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var p

    var body: some View {
        ScrollView(showsIndicators: false) {
            VStack(alignment: .leading, spacing: 14) {
                SheetHeader(title: "生词本") { dismiss() }
                if store.vocab.isEmpty {
                    Text("还没有生词。播放时点原文里的词，在弹出的卡片上点 ☆ 收藏。")
                        .font(Fonts.ui(14, .medium)).foregroundStyle(p.t2)
                        .padding(.top, 40)
                }
                ForEach(store.vocab, id: \.word) { v in
                    VStack(alignment: .leading, spacing: 6) {
                        HStack(alignment: .firstTextBaseline, spacing: 10) {
                            Text(v.word).font(Fonts.display(22))
                            if !v.phonetic.isEmpty { Text("/\(v.phonetic)/").font(Fonts.ipa(15)).foregroundStyle(p.violetText) }
                            Spacer()
                            IconButton(icon: .trash, size: 17, color: p.t2, label: "移出生词本") { store.removeVocab(v.word) }
                                .frame(width: 32, height: 32)
                        }
                        if !v.meaning.isEmpty { Text(v.meaning).font(Fonts.ui(14, .medium)).foregroundStyle(p.t1) }
                        Text(v.sentence).font(Fonts.ui(14, .medium)).foregroundStyle(p.t2)
                        if let id = v.articleId, let s = v.sentenceIndex {
                            Button {
                                store.play(id)
                                store.seekSentence(Int(s))
                                dismiss()
                            } label: {
                                Label("\(v.articleTitle ?? "原文") · 第 \(s + 1) 句", systemImage: "speaker.wave.2")
                                    .font(Fonts.ui(13, .bold)).foregroundStyle(p.accentText)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(16)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(p.card, in: RoundedRectangle(cornerRadius: 20, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 20, style: .continuous).strokeBorder(p.line, lineWidth: 1))
                }
            }
            .padding(.horizontal, 20).padding(.top, 20).padding(.bottom, 30)
        }
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
        .presentationCornerRadius(28)
        .presentationBackground(p.bg)
    }
}

// MARK: - 术语表

struct GlossarySheet: View {
    @Environment(Store.self) private var store
    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var p
    @State private var term = ""
    @State private var zh = ""

    var body: some View {
        ScrollView(showsIndicators: false) {
            VStack(alignment: .leading, spacing: 14) {
                SheetHeader(title: "术语表") { dismiss() }
                GroupCard {
                    ForEach(store.glossary, id: \.term) { g in
                        HStack(spacing: 10) {
                            Text(g.term).font(Fonts.ui(15, .bold))
                            Text("→").foregroundStyle(p.t2)
                            Text(g.translation).font(Fonts.ui(15, .medium))
                            Spacer()
                            IconButton(icon: .close, size: 15, weight: 2, color: p.t2, label: "删除术语 \(g.term)") {
                                store.removeGlossary(g.term)
                            }
                            .frame(width: 32, height: 32)
                        }
                        .frame(minHeight: 50)
                        .overlay(alignment: .bottom) { Rectangle().fill(p.line).frame(height: 1) }
                    }
                    HStack(spacing: 10) {
                        TextField("英文术语，如 token", text: $term)
                            .textInputAutocapitalization(.never).autocorrectionDisabled()
                        TextField("固定译法，如 令牌", text: $zh)
                        Button {
                            if store.addGlossary(term: term, translation: zh) { term = ""; zh = "" }
                        } label: { Chip("添加", kind: .accent) }
                        .buttonStyle(.plain)
                        .disabled(term.trimmingCharacters(in: .whitespaces).isEmpty || zh.trimmingCharacters(in: .whitespaces).isEmpty)
                    }
                    .font(Fonts.ui(15, .medium))
                    .frame(minHeight: 54)
                }
                Text("整句翻译遇到术语时一律使用固定译法；修改后含该术语的句子会自动重新翻译。")
                    .font(Fonts.ui(12, .medium)).foregroundStyle(p.t2)
            }
            .padding(.horizontal, 20).padding(.top, 20).padding(.bottom, 30)
        }
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
        .presentationCornerRadius(28)
        .presentationBackground(p.bg)
    }
}
