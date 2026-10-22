import SpeakerCore
import SwiftUI
import UniformTypeIdentifiers

/// 导入（UI_SPEC §4.6）：关闭 · “导入文章” · 分段（粘贴文本 / 文件 / 剪贴板）· 标题 · 正文 ·
/// 音色 + “导入后立即播放” · “共 N 句 · 预计约 N 秒 · 离线转换，不上传” · “开始转换”。
/// 确定后立刻出现在资料库并开始合成（边听边转）。
struct ImportSheet: View {
    enum Source: Hashable { case paste, file, clipboard }

    @Environment(Store.self) private var store
    @Environment(Prefs.self) private var prefs
    @Environment(AppUI.self) private var ui
    @Environment(\.dismiss) private var dismiss
    @Environment(\.palette) private var p
    @State private var source: Source = .paste
    @State private var title = ""
    @State private var text = ""
    @State private var fileName: String?
    @State private var showFiles = false
    @State private var playNow = true
    @State private var voice: String = Store.defaultVoice
    enum Field { case title, body }
    @FocusState private var focused: Field?

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SheetHeader(title: "导入文章") { dismiss() }
            Segmented(options: [(Source.paste, "粘贴文本", nil), (.file, "文件", nil), (.clipboard, "剪贴板", nil)],
                      selection: $source, importStyle: true, height: 38)
            VStack(alignment: .leading, spacing: 8) {
                LabelText("标题（可选）")
                TextField("", text: $title, prompt: Text("默认取第一句").foregroundColor(p.t2))
                    .font(Fonts.ui(15, .semibold))
                    .padding(.horizontal, 14)
                    .frame(height: 48)
                    .background(p.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(p.line, lineWidth: 1))
                    .focused($focused, equals: .title)
                    .accessibilityIdentifier("import-title")
            }
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    LabelText("正文")
                    Spacer()
                    if let fileName { Text(fileName).font(Fonts.ui(12)).foregroundStyle(p.t2).lineLimit(1) }
                }
                ZStack(alignment: .topLeading) {
                    TextEditor(text: $text)
                        .font(Fonts.ui(15, .medium))
                        .lineSpacing(24 - 20.5)
                        .scrollContentBackground(.hidden)
                        .padding(.horizontal, 10).padding(.vertical, 8)
                        .focused($focused, equals: .body)
                        .accessibilityIdentifier("import-body")
                    if text.isEmpty {
                        Text("在这里粘贴英文文章…").font(Fonts.ui(15, .medium)).foregroundStyle(p.t2)
                            .padding(.horizontal, 15).padding(.vertical, 16)
                            .allowsHitTesting(false)
                    }
                }
                .frame(minHeight: 180, maxHeight: .infinity)
                .background(p.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(p.line, lineWidth: 1))
            }
            GroupCard(hPad: 14) {
                GroupRow(title: "音色", height: 52) {
                    Menu {
                        ForEach(store.voices, id: \.id) { v in Button(Fmt.voice(v)) { voice = v.id } }
                    } label: {
                        Chip(voiceLabel + " ›", kind: .neutral)
                    }
                }
                GroupRow(title: "导入后立即播放", divider: false, height: 52) {
                    AccentSwitch(isOn: $playNow, label: "导入后立即播放")
                }
            }
            Text(stats).font(Fonts.ui(13, .medium)).foregroundStyle(p.t2).frame(maxWidth: .infinity)
            PrimaryButton(title: "开始转换", enabled: !trimmed.isEmpty) { commit() }
                .accessibilityIdentifier("import-commit")
        }
        .padding(.horizontal, 20)
        .padding(.top, 24)
        .padding(.bottom, 12)
        // 键盘弹出时不压缩布局（正文框在上半部分，仍可见）；键盘上方给“完成”
        .ignoresSafeArea(.keyboard, edges: .bottom)
        .toolbar {
            ToolbarItemGroup(placement: .keyboard) {
                Spacer()
                Button("完成") { focused = nil }.font(Fonts.ui(15, .bold))
            }
        }
        .presentationDetents([.large])
        .presentationDragIndicator(.visible)
        .presentationCornerRadius(28)
        .presentationBackground(p.surface)
        .onChange(of: source) { _, s in
            switch s {
            case .file: showFiles = true
            case .clipboard: if let s = UIPasteboard.general.string { text = s; fileName = nil }
            case .paste: break
            }
        }
        .fileImporter(isPresented: $showFiles, allowedContentTypes: [.plainText, .text, .utf8PlainText]) { result in
            guard case let .success(url) = result else { source = .paste; return }
            let scoped = url.startAccessingSecurityScopedResource()
            defer { if scoped { url.stopAccessingSecurityScopedResource() } }
            if let s = try? String(contentsOf: url, encoding: .utf8) {
                text = s
                fileName = url.lastPathComponent
                if title.isEmpty { title = url.deletingPathExtension().lastPathComponent }
            }
        }
        .onAppear {
            voice = prefs.defaultVoice
            let d = UserDefaults.standard
            if let t = d.string(forKey: "importTitle") { title = t }
            if let b = d.string(forKey: "importBody") { text = b }
            if d.bool(forKey: "importCommit") {
                DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { commit() }
            }
        }
    }

    private var trimmed: String { text.trimmingCharacters(in: .whitespacesAndNewlines) }

    private var voiceLabel: String {
        store.voices.first { $0.id == voice }.map { Fmt.voice($0) } ?? voice
    }

    /// 句数是客户端估算（核心没有“预分句”接口）；时长按约 2.6 词/秒估算。
    private var stats: String {
        let words = trimmed.split(whereSeparator: \.isWhitespace).count
        guard words > 0 else { return "支持纯文本与 Markdown · 离线转换，不上传" }
        var sentences = 0
        trimmed.enumerateSubstrings(in: trimmed.startIndex..., options: .bySentences) { _, _, _, _ in sentences += 1 }
        let secs = max(1, Int((Double(words) / 2.6).rounded()))
        let dur = secs < 90 ? "\(secs) 秒" : "\(Int((Double(secs) / 60).rounded())) 分钟"
        return "共 \(max(1, sentences)) 句 · 预计约 \(dur) · 离线转换，不上传"
    }

    private func commit() {
        guard !trimmed.isEmpty else { return }
        let t = title.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let a = store.importText(title: t.isEmpty ? nil : t, body: text,
                                       source: fileName == nil ? .paste : .file, sourceRef: fileName,
                                       voice: voice) else { return }
        if playNow {
            store.play(a.id)
            dismiss()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.45) { store.isPlayerExpanded = true }
        } else {
            ui.select(.library)
            dismiss()
        }
    }
}
