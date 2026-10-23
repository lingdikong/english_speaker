#!/usr/bin/env python3
"""生成 macOS 应用图标 AppIcon.icns（品牌：圆角方块内 5 根声波竖条）。

用法：
  scripts/make-appicon.py   # 输出 crates/es-desktop/assets/AppIcon.icns

设计对齐 ui/web/src/components/Brand.svelte：青绿渐变底 + 墨色声波条，
版式按 Apple 图标模板：1024 画布中央 824×824 圆角方块（radius 185.4）。
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
OUT_ICNS = ROOT / "crates/es-desktop/assets/AppIcon.icns"

BASE = 1024
# Apple 模板：图标主体 824，边距 100，圆角半径 185.4
PAD = 100
SQUARE = BASE - PAD * 2
RADIUS = 185.4

# 品牌色（ui/web/src/app.css）：浅色主题 accent 上下渐变
TOP = (94, 229, 207)      # 比 #3ccfb9 略亮的顶光
BOTTOM = (23, 158, 140)   # 趋近暗色主题的 #0e9483
INK = (14, 26, 24)        # --accent-ink #0e1a18

# 声波条：Brand 的 H=[8,15,20,12,7]（满幅 20），等比放大
BAR_HEIGHTS = [8, 15, 20, 12, 7]
BAR_MAX = 460             # 最高条像素高
BAR_W = 64                # 条宽
BAR_GAP = 44              # 条间距


def rounded_gradient(size: int) -> Image.Image:
    """824 圆角方块，垂直渐变底 + 顶部受光高光。"""
    ss = size * 2  # 2x 超采样抗锯齿
    img = Image.new("RGBA", (ss, ss), (0, 0, 0, 0))
    grad = Image.new("RGBA", (ss, ss))
    px = grad.load()
    for y in range(ss):
        t = y / (ss - 1)
        r = round(TOP[0] + (BOTTOM[0] - TOP[0]) * t)
        g = round(TOP[1] + (BOTTOM[1] - TOP[1]) * t)
        b = round(TOP[2] + (BOTTOM[2] - TOP[2]) * t)
        for x in range(ss):
            px[x, y] = (r, g, b, 255)
    mask = Image.new("L", (ss, ss), 0)
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, ss - 1, ss - 1), radius=round(RADIUS * 2), fill=255
    )
    img.paste(grad, (0, 0), mask)

    # 顶部高光：上沿 18% 叠一层由白渐透明的竖向光，模拟受光面
    hl = Image.new("RGBA", (ss, ss), (0, 0, 0, 0))
    hpx = hl.load()
    hl_h = round(ss * 0.18)
    for y in range(hl_h):
        a = round(46 * (1 - y / hl_h))
        for x in range(ss):
            hpx[x, y] = (255, 255, 255, a)
    hl.putalpha(Image.composite(hl.split()[3], Image.new("L", (ss, ss), 0), mask))
    img.alpha_composite(hl)
    return img.resize((size, size), Image.LANCZOS)


def wave_bars(size: int) -> Image.Image:
    """居中的 5 根声波条（墨色，圆头），沿水平中线对称分布。"""
    ss = size * 2
    img = Image.new("RGBA", (ss, ss), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    total_w = 5 * BAR_W + 4 * BAR_GAP
    x0 = (size - total_w) / 2
    scale = BAR_MAX / max(BAR_HEIGHTS)
    # 全部坐标统一在 ss（2x 超采样）域里
    w, gap = BAR_W * 2, BAR_GAP * 2
    x = x0 * 2
    for h in BAR_HEIGHTS:
        bh = h * scale * 2
        top = (ss - bh) / 2
        d.rounded_rectangle((x, top, x + w, top + bh), radius=w / 2, fill=INK + (255,))
        x += w + gap
    return img.resize((size, size), Image.LANCZOS)


def main() -> int:
    with tempfile.TemporaryDirectory() as td:
        iconset = Path(td) / "AppIcon.iconset"
        iconset.mkdir()
        for sz in (16, 32, 64, 128, 256, 512, 1024):
            icon = rounded_gradient(sz)
            icon.alpha_composite(wave_bars(sz))
            name = f"icon_{sz}x{sz}.png" if sz < 1024 else "icon_512x512@2x.png"
            icon.save(iconset / name)
            if sz >= 32:
                half = icon.resize((sz // 2, sz // 2), Image.LANCZOS)
                half.save(iconset / f"icon_{sz // 2}x{sz // 2}.png")
        OUT_ICNS.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            ["iconutil", "-c", "icns", str(iconset), "-o", str(OUT_ICNS)], check=True
        )
    print(f"icns written: {OUT_ICNS}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
