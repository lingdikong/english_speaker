import SwiftUI
import UIKit

/// 确定性声波（UI_SPEC §3.1，与设计稿 `seedOf` / `bars` 逐位一致，三端相同）。
enum WaveAlgo {
    /// FNV-1a 32，按 UTF-16 码元（与 JS `charCodeAt` 一致）。
    static func seed(_ s: String) -> UInt32 {
        var h: UInt32 = 2_166_136_261
        for c in s.utf16 {
            h ^= UInt32(c)
            h = h &* 16_777_619
        }
        return h
    }

    static func bars(seed: UInt32, n: Int, min lo: Int, max hi: Int) -> [CGFloat] {
        var x: UInt32 = seed == 0 ? 1 : seed
        var out: [CGFloat] = []
        out.reserveCapacity(n)
        for i in 0..<n {
            x = x &* 1_103_515_245 &+ 12345
            let r = Double(x >> 8) / 16_777_216
            let e = sin(Double.pi * (Double(i) + 0.5) / Double(n))
            let v = Double(lo) + Double(hi - lo) * Swift.min(1, 0.3 * r + 0.7 * r * e + 0.15 * e)
            out.append(CGFloat(floor(v + 0.5))) // JS Math.round
        }
        return out
    }

    static func bars(_ key: String, n: Int, min: Int, max: Int) -> [CGFloat] {
        bars(seed: seed(key), n: n, min: min, max: max)
    }
}

/// 一排竖条（条宽平分，flex:1）。
struct WaveBars: View {
    let heights: [CGFloat]
    var maxHeight: CGFloat
    var gap: CGFloat = 2
    var bottom = false
    var color: (Int) -> Color

    var body: some View {
        HStack(alignment: bottom ? .bottom : .center, spacing: gap) {
            ForEach(heights.indices, id: \.self) { i in
                RoundedRectangle(cornerRadius: 2, style: .continuous)
                    .fill(color(i))
                    .frame(maxWidth: .infinity)
                    .frame(height: heights[i])
            }
        }
        .frame(height: maxHeight, alignment: bottom ? .bottom : .center)
    }
}

/// 生成封面：card 底 + line 内描边，圆角 = 边长/4.5，内部声波（UI_SPEC §3.1）。
struct WaveCover: View {
    let title: String
    let size: CGFloat
    var violet = false
    @Environment(\.palette) private var p

    static func barCount(_ s: CGFloat) -> Int {
        switch s {
        case 64...: 11
        case 52...: 9
        case 43...: 7
        default: 6
        }
    }

    var body: some View {
        let s = Int(size)
        let lo = Int((Double(s) / 10).rounded()), hi = s - Int((Double(s) / 2.6).rounded())
        let r = (size / 4.5).rounded()
        WaveBars(heights: WaveAlgo.bars(title, n: Self.barCount(size), min: lo, max: hi),
                 maxHeight: CGFloat(hi), color: { _ in violet ? p.violet : p.accent })
            .padding(.horizontal, (size / 7).rounded())
            .frame(width: size, height: size)
            .background(p.card, in: RoundedRectangle(cornerRadius: r, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: r, style: .continuous).strokeBorder(p.line, lineWidth: 1))
            .accessibilityHidden(true)
    }
}

/// 圆形封面 + 进度环（底部播放条 46px）。
struct RingCover: View {
    let title: String
    let progress: Double
    @Environment(\.palette) private var p

    var body: some View {
        ZStack {
            Circle().stroke(p.waveE, lineWidth: 2.5).frame(width: 43, height: 43)
            Circle().trim(from: 0, to: max(0, min(1, progress)))
                .stroke(p.accent, style: StrokeStyle(lineWidth: 2.5, lineCap: .round))
                .rotationEffect(.degrees(-90))
                .frame(width: 43, height: 43)
            WaveBars(heights: WaveAlgo.bars(title, n: 5, min: 4, max: 18), maxHeight: 18, color: { _ in p.accent })
                .padding(.horizontal, 8)
                .frame(width: 36, height: 36)
                .background(p.card, in: Circle())
        }
        .frame(width: 46, height: 46)
        .accessibilityHidden(true)
    }
}

/// 声波进度条（UI_SPEC §3.2）：已播 accent / 已缓冲 wave-b / 未合成 wave-e，底对齐；点按/拖动 → seek。
struct WaveProgress: View {
    let key: String
    let played: Double
    let buffered: Double
    var n = 56
    var height: CGFloat = 32
    var gap: CGFloat = 3
    var onSeek: ((Double) -> Void)?
    @Environment(\.palette) private var p
    @State private var dragFrac: Double?

    var body: some View {
        let heights = WaveAlgo.bars(key, n: n, min: 6, max: Int(height))
        let pf = dragFrac ?? played
        let playedBars = Int((pf * Double(n)).rounded())
        let bufBars = Int((buffered * Double(n)).rounded())
        GeometryReader { geo in
            WaveBars(heights: heights, maxHeight: height, gap: gap, bottom: true) { i in
                i < playedBars ? p.accent : (i < max(bufBars, playedBars) ? p.waveB : p.waveE)
            }
            .contentShape(Rectangle())
            .gesture(DragGesture(minimumDistance: 0)
                .onChanged { v in dragFrac = max(0, min(1, v.location.x / geo.size.width)) }
                .onEnded { v in
                    let f = max(0, min(1, v.location.x / geo.size.width))
                    dragFrac = nil
                    onSeek?(f)
                })
        }
        .frame(height: height)
        .accessibilityElement()
        .accessibilityLabel("播放进度")
        .accessibilityValue("\(Int(played * 100))%")
        .accessibilityAdjustableAction { dir in
            onSeek?(max(0, min(1, played + (dir == .increment ? 0.05 : -0.05))))
        }
    }
}

enum CoverRenderer {
    /// 锁屏 Now Playing 用的封面位图（夜间配色的声波封面）。
    @MainActor
    static func image(title: String, size: CGFloat = 600) -> UIImage? {
        let renderer = ImageRenderer(content:
            WaveCover(title: title, size: 72)
                .environment(\.palette, .dark())
                .scaleEffect(size / 72)
                .frame(width: size, height: size)
                .background(Palette.dark().bg))
        renderer.scale = 1
        return renderer.uiImage
    }
}
