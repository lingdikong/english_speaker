import SpeakerCore
import SwiftUI

/// 待播清单（UI_SPEC §4.5）：“正在播放” + 待播列表（拖动排序、左滑删除）；当前播完自动播下一篇（客户端队列）。
struct UpNextSheet: View {
    @Environment(Store.self) private var store
    @Environment(\.palette) private var p
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        @Bindable var store = store
        VStack(alignment: .leading, spacing: 0) {
            SheetHeader(title: "待播清单") { dismiss() }
                .padding(.horizontal, 20).padding(.top, 20).padding(.bottom, 8)
            List {
                if let a = store.current {
                    Section {
                        row(a, playing: true)
                    } header: { LabelText("正在播放") }
                }
                Section {
                    if store.upNext.isEmpty {
                        Text("在资料库长按文章或点 ⋯ →“加入待播清单”")
                            .font(Fonts.ui(14)).foregroundStyle(p.t2)
                            .listRowBackground(Color.clear)
                    }
                    ForEach(store.upNext, id: \.self) { id in
                        if let a = store.article(id) { row(a, playing: false) }
                    }
                    .onMove { store.upNext.move(fromOffsets: $0, toOffset: $1) }
                    .onDelete { store.upNext.remove(atOffsets: $0) }
                } header: {
                    LabelText("接下来 · \(store.upNext.count) 篇")
                } footer: {
                    Text("当前文章播完后自动播放下一篇。按住拖动排序，左滑移除。")
                        .font(Fonts.ui(12)).foregroundStyle(p.t2)
                }
            }
            .listStyle(.insetGrouped)
            .scrollContentBackground(.hidden)
            .environment(\.defaultMinListRowHeight, 64)
        }
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
        .presentationCornerRadius(28)
        .presentationBackground(p.surface)
    }

    private func row(_ a: ArticleSummary, playing: Bool) -> some View {
        HStack(spacing: 12) {
            WaveCover(title: a.title, size: 48, violet: !playing)
            VStack(alignment: .leading, spacing: 3) {
                Text(a.title).font(Fonts.ui(15, .bold)).lineLimit(1)
                    .foregroundStyle(playing ? p.accentText : p.t1)
                HStack(spacing: 8) {
                    Text(store.rowMeta(a)).font(Fonts.ui(13, .medium)).foregroundStyle(p.t2).lineLimit(1)
                    StatusChip(article: a)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.vertical, 4)
        .listRowBackground(p.card)
        .listRowSeparatorTint(p.line)
        .contentShape(Rectangle())
        .onTapGesture {
            if !playing {
                store.upNext.removeAll { $0 == a.id }
                store.play(a.id)
            }
        }
    }
}
