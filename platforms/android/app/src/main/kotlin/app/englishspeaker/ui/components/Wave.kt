package app.englishspeaker.ui.components

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.englishspeaker.ui.theme.LocalPalette
import kotlin.math.PI
import kotlin.math.floor
import kotlin.math.min
import kotlin.math.sin

/**
 * 确定性声纹（UI_SPEC §3.1，与设计稿 `seedOf / bars` 完全一致，三端一致）。
 * 全部用无符号 32 位运算：`Math.imul(...)>>>0` 即 UInt 乘法取低 32 位。
 */
object Wave {
    /** FNV-1a 32 位；按 UTF-16 码元（与 JS `charCodeAt` 相同）。 */
    fun seedOf(s: String): UInt {
        var h = 2166136261u
        for (ch in s) {
            h = h xor ch.code.toUInt()
            h *= 16777619u
        }
        return h
    }

    /** JS `Math.round`：四舍五入（.5 向正无穷）。 */
    fun jsRound(v: Double): Int = floor(v + 0.5).toInt()

    fun bars(seed: UInt, n: Int, min: Int, max: Int): IntArray {
        var x = if (seed == 0u) 1u else seed
        return IntArray(n) { i ->
            x = x * 1103515245u + 12345u
            val r = (x shr 8).toDouble() / 16777216.0
            val e = sin(PI * (i + 0.5) / n)
            jsRound(min + (max - min) * min(1.0, 0.3 * r + 0.7 * r * e + 0.15 * e))
        }
    }

    /** 缩略图根数：72→11，56→9，48→7，38→6。 */
    fun countFor(size: Int): Int = when {
        size >= 72 -> 11
        size >= 56 -> 9
        size >= 48 -> 7
        else -> 6
    }
}

/**
 * 生成封面：`card` 底 + `line` 1px 内描边，圆角 = 边长 / 4.5，内部一排竖条声波（垂直居中）。
 * 参数：min = round(s/10)，max = s − round(s/2.6)，水平内边距 round(s/7)，条间距 2，条圆角 2。
 */
@Composable
fun WaveCover(title: String, size: Int, color: Color, modifier: Modifier = Modifier) {
    val p = LocalPalette.current
    val n = Wave.countFor(size)
    val min = Wave.jsRound(size / 10.0)
    val max = size - Wave.jsRound(size / 2.6)
    val pad = Wave.jsRound(size / 7.0)
    val hs = remember(title, size) { Wave.bars(Wave.seedOf(title), n, min, max) }
    val shape = RoundedCornerShape(Wave.jsRound(size / 4.5).dp)
    Box(
        modifier
            .size(size.dp)
            .background(p.card, shape)
            .border(1.dp, p.line, shape),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.size(size.dp, max.dp)) {
            drawBars(hs, max, padPx = pad.dp.toPx(), gapPx = 2.dp.toPx(), radiusPx = 2.dp.toPx(), bottom = false) { color }
        }
    }
}

/** 竖条：flex:1 均分宽度（扣除内边距与间距），居中或底对齐。 */
internal fun androidx.compose.ui.graphics.drawscope.DrawScope.drawBars(
    hs: IntArray, max: Int, padPx: Float, gapPx: Float, radiusPx: Float, bottom: Boolean, color: (Int) -> Color,
) {
    val n = hs.size
    val w = (size.width - padPx * 2 - gapPx * (n - 1)) / n
    val unit = size.height / max
    for (i in 0 until n) {
        val h = hs[i] * unit
        val x = padPx + i * (w + gapPx)
        val y = if (bottom) size.height - h else (size.height - h) / 2
        drawRoundRect(color(i), Offset(x, y), Size(w, h), CornerRadius(radiusPx, radiusPx))
    }
}

/**
 * 声波进度条（UI_SPEC §3.2）：种子为文章 id；已播放 accent，已缓冲 wave-b，未合成 wave-e；底对齐。
 * 点按 / 拖动 → [onSeek]（0..1）。
 */
@Composable
fun WaveProgress(
    seed: String,
    played: Float,
    buffered: Float,
    modifier: Modifier = Modifier,
    n: Int = 56,
    height: Dp = 32.dp,
    gap: Dp = 3.dp,
    min: Int = 6,
    onSeek: ((Float) -> Unit)? = null,
) {
    val p = LocalPalette.current
    val max = height.value.toInt()
    val hs = remember(seed, n, max) { Wave.bars(Wave.seedOf(seed), n, min, max) }
    // 拖动时只预览，松手再 seek
    var drag by remember { mutableStateOf<Float?>(null) }
    val shown = drag ?: played
    val playedN = Wave.jsRound((shown.coerceIn(0f, 1f) * n).toDouble())
    val bufferedN = Wave.jsRound((buffered.coerceIn(0f, 1f) * n).toDouble())
    var m = modifier.fillMaxWidth().height(height)
    if (onSeek != null) {
        val seek by rememberUpdatedState(onSeek)
        m = m
            .pointerInput(Unit) { detectTapGestures { seek((it.x / size.width).coerceIn(0f, 1f)) } }
            .pointerInput(Unit) {
                detectHorizontalDragGestures(
                    onDragEnd = { drag?.let { seek(it) }; drag = null },
                    onDragCancel = { drag = null },
                ) { change, _ ->
                    change.consume()
                    drag = (change.position.x / size.width).coerceIn(0f, 1f)
                }
            }
    }
    Canvas(m) {
        drawBars(hs, max, 0f, gap.toPx(), 2.dp.toPx(), bottom = true) { i ->
            when {
                i < playedN -> p.accent
                i < bufferedN -> p.waveB
                else -> p.waveE
            }
        }
    }
}
