import SpeakerCore
import SwiftUI

/// 搜索（UI_SPEC §4.8）：按标题与正文过滤（客户端过滤 list_articles + get_article），命中词用 accent-soft 标出。
struct SearchView: View {
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui
    @Environment(\.palette) private var p
    @State private var query = UserDefaults.standard.string(forKey: "searchQuery") ?? ""
    @FocusState private var focused: Bool

    var body: some View {
        TabScroll(tab: .search) {
            VStack(alignment: .leading, spacing: 20) {
                LargeTitle("搜索")
                field
                let q = query.trimmingCharacters(in: .whitespacesAndNewlines)
                let results = q.isEmpty ? store.articles : store.articles.filter { matches($0, q) }
                VStack(alignment: .leading, spacing: 0) {
                    Text(q.isEmpty ? "全部文章" : "\(results.count) 个结果")
                        .font(Fonts.display(20, .semibold)).padding(.bottom, 4)
                    if results.isEmpty {
                        Text("没有找到“\(q)”").font(Fonts.ui(14)).foregroundStyle(p.t2).padding(.vertical, 28)
                    }
                    ForEach(Array(results.enumerated()), id: \.element.id) { i, a in
                        ArticleRow(article: a, index: i, context: results,
                                   titleText: q.isEmpty ? nil : highlight(a.title, q),
                                   snippet: q.isEmpty ? nil : snippet(a, q))
                    }
                }
            }
        }
        .onChange(of: ui.tab) { _, t in if t != .search { focused = false } }
    }

    private var field: some View {
        HStack(spacing: 10) {
            IconView(.search, size: 18, weight: 2).foregroundStyle(p.t2)
            TextField("", text: $query, prompt: Text("搜索文章标题与原文").foregroundColor(p.t2))
                .font(Fonts.ui(15, .semibold))
                .focused($focused)
                .submitLabel(.search)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
            if !query.isEmpty {
                Button { query = "" } label: { IconView(.close, size: 16, weight: 2).foregroundStyle(p.t2) }
                    .buttonStyle(.plain).accessibilityLabel("清除")
            }
        }
        .padding(.horizontal, 14)
        .frame(height: 48)
        .background(p.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(p.line, lineWidth: 1))
    }

    private func matches(_ a: ArticleSummary, _ q: String) -> Bool {
        a.title.localizedCaseInsensitiveContains(q) || (store.bodies[a.id]?.localizedCaseInsensitiveContains(q) ?? false)
    }

    private func highlight(_ s: String, _ q: String) -> AttributedString {
        var out = AttributedString(s)
        var search = s.startIndex..<s.endIndex
        while let r = s.range(of: q, options: [.caseInsensitive, .diacriticInsensitive], range: search) {
            if let ar = Range(r, in: out) {
                out[ar].backgroundColor = p.accentSoft
                out[ar].foregroundColor = p.accentText
            }
            search = r.upperBound..<s.endIndex
        }
        return out
    }

    /// 正文中第一处命中前后的片段。
    private func snippet(_ a: ArticleSummary, _ q: String) -> AttributedString? {
        guard let body = store.bodies[a.id],
              let r = body.range(of: q, options: [.caseInsensitive, .diacriticInsensitive]) else { return nil }
        let start = body.index(r.lowerBound, offsetBy: -36, limitedBy: body.startIndex) ?? body.startIndex
        let end = body.index(r.upperBound, offsetBy: 60, limitedBy: body.endIndex) ?? body.endIndex
        var text = String(body[start..<end]).replacingOccurrences(of: "\n", with: " ")
        if start > body.startIndex { text = "…" + text }
        if end < body.endIndex { text += "…" }
        return highlight(text, q)
    }
}
