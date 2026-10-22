import SpeakerCore
import SwiftUI

/// 首页（UI_SPEC §4.3）：大标题；继续收听；最近添加。无内容时显示空状态。
struct HomeView: View {
    @Environment(Store.self) private var store
    @Environment(AppUI.self) private var ui
    @Environment(\.palette) private var p

    var body: some View {
        TabScroll(tab: .home) {
            VStack(alignment: .leading, spacing: 20) {
                HStack {
                    LargeTitle("首页")
                    Spacer()
                    BrandMark(size: 36)
                }
                if store.articles.isEmpty {
                    empty
                } else {
                    if let a = store.barArticle { ContinueCard(article: a) }
                    VStack(alignment: .leading, spacing: 0) {
                        Text("最近添加").font(Fonts.display(20, .semibold)).padding(.bottom, 4)
                        let recent = store.articles.sorted { $0.createdAtMs > $1.createdAtMs }.prefix(8)
                        ForEach(Array(recent.enumerated()), id: \.element.id) { i, a in
                            ArticleRow(article: a, index: i, context: Array(recent)).id(a.id)
                        }
                    }
                }
            }
        }
    }

    private var empty: some View {
        VStack(spacing: 18) {
            BrandMark(size: 72)
            Text("把英文文章变成可以边听边读的播客").font(Fonts.ui(15)).foregroundStyle(p.t2)
                .multilineTextAlignment(.center)
            PrimaryButton(title: "导入第一篇文章", height: 50, fontSize: 15) { ui.showImport = true }
                .frame(width: 220)
            Text("完全离线 · 文章不会上传").font(Fonts.ui(12)).foregroundStyle(p.t2)
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 120)
    }
}
