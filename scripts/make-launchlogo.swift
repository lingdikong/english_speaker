import Cocoa
import CoreText

// 渲染 iOS 启动屏 Logo：波形图标 + "English Speaker" 品牌字标。
// 输出浅色/深色两套 @3x PNG，供 asset catalog 使用。
// 使用 CoreGraphics 原生左下原点坐标系，不做整体翻转，避免文字镜像。

let args = CommandLine.arguments
guard args.count >= 3 else {
    FileHandle.standardError.write("usage: gen.swift <fontPath> <outDir>\n".data(using: .utf8)!)
    exit(1)
}
let fontPath = args[1]
let outDir = args[2]

// 注册品牌字体
let fontURL = URL(fileURLWithPath: fontPath)
var err: Unmanaged<CFError>?
if !CTFontManagerRegisterFontsForURL(fontURL as CFURL, .process, &err) {
    print("font register note: \(String(describing: err?.takeUnretainedValue()))")
}

// ---- 设计参数（pt 单位，最终按 scale 放大） ----
let scale: CGFloat = 3.0
let iconH: CGFloat = 116          // 波形图标总高
let barRatio: [CGFloat] = [0.48, 0.71, 1.0, 0.71, 0.48]
let barW = iconH * 0.16           // 单根条宽
let barGap = iconH * 0.11         // 条间距
let iconTextGap: CGFloat = 36     // 图标与字标间距
let textSize: CGFloat = 33        // 字标字号
let tracking: CGFloat = 0.4       // 字距

func barsWidth() -> CGFloat {
    return barW * CGFloat(barRatio.count) + barGap * CGFloat(barRatio.count - 1)
}

func cgColor(_ hex: UInt32) -> CGColor {
    CGColor(red: CGFloat((hex >> 16) & 0xFF) / 255,
            green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
}

func render(accentHex: UInt32, textHex: UInt32, outName: String) {
    let font = CTFontCreateWithName("Space Grotesk Bold" as CFString, textSize, nil)
    let text = "English Speaker"
    let probeAttrs: [NSAttributedString.Key: Any] = [.font: font, .kern: tracking]
    let probeLine = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: probeAttrs))
    let textBounds = CTLineGetBoundsWithOptions(probeLine, .useOpticalBounds)
    let textW = textBounds.width
    let textH = textBounds.height

    let contentW = max(barsWidth(), textW)
    let contentH = iconH + iconTextGap + textH
    let pad: CGFloat = 8
    let canvasW = contentW + pad * 2
    let canvasH = contentH + pad * 2

    let pxW = Int((canvasW * scale).rounded())
    let pxH = Int((canvasH * scale).rounded())

    let colorSpace = CGColorSpaceCreateDeviceRGB()
    guard let ctx = CGContext(data: nil, width: pxW, height: pxH,
                              bitsPerComponent: 8, bytesPerRow: 0,
                              space: colorSpace,
                              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
        fatalError("no ctx")
    }
    ctx.scaleBy(x: scale, y: scale)
    // 左下原点，y 向上。内容从底部 pad 开始向上排：文字在底，图标在上。

    // ---- 字标（底部，水平居中） ----
    let attrs: [NSAttributedString.Key: Any] = [
        .font: font,
        .kern: tracking,
        .foregroundColor: NSColor(cgColor: cgColor(textHex))!,
    ]
    let line = CTLineCreateWithAttributedString(NSAttributedString(string: text, attributes: attrs))
    // baseline：底部 pad + (textH 块内，使光学边界贴合) —— 直接用 descent 对齐底部
    let descent = CTFontGetDescent(font)
    let textX = (canvasW - textBounds.width) / 2 - textBounds.minX
    let baseY = pad + descent
    ctx.textPosition = CGPoint(x: textX, y: baseY)
    CTLineDraw(line, ctx)

    // ---- 波形图标（文字上方，水平居中） ----
    let accent = cgColor(accentHex)
    let totalBW = barsWidth()
    var x = (canvasW - totalBW) / 2
    let iconBottom = pad + textH + iconTextGap
    let iconCenterY = iconBottom + iconH / 2
    for r in barRatio {
        let h = iconH * r
        let rect = CGRect(x: x, y: iconCenterY - h / 2, width: barW, height: h)
        let path = CGPath(roundedRect: rect, cornerWidth: barW / 2, cornerHeight: barW / 2, transform: nil)
        ctx.addPath(path)
        ctx.setFillColor(accent)
        ctx.fillPath()
        x += barW + barGap
    }

    guard let img = ctx.makeImage() else { fatalError("no image") }
    let outURL = URL(fileURLWithPath: outDir).appendingPathComponent(outName)
    guard let dest = CGImageDestinationCreateWithURL(outURL as CFURL, "public.png" as CFString, 1, nil) else {
        fatalError("no dest")
    }
    CGImageDestinationAddImage(dest, img, nil)
    CGImageDestinationFinalize(dest)
    print("wrote \(outURL.path)  \(pxW)x\(pxH)px  (canvas \(canvasW)x\(canvasH)pt @\(Int(scale))x)")
}

render(accentHex: 0x3CCFB9, textHex: 0xE6E9EF, outName: "LaunchLogo-dark.png")
render(accentHex: 0x0E9483, textHex: 0x1E232E, outName: "LaunchLogo-light.png")
