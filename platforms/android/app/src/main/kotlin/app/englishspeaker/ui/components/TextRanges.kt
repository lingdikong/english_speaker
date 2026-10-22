package app.englishspeaker.ui.components

import androidx.compose.foundation.text.InlineTextContent
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle

/** 文本区间 [start, end) 在每一行上的外接矩形（用于胶囊底色、点线下划线）。 */
fun TextLayoutResult.rangeRects(start: Int, end: Int): List<Rect> {
    if (end <= start || start >= layoutInput.text.length) return emptyList()
    val e = end.coerceAtMost(layoutInput.text.length)
    val first = getLineForOffset(start)
    val last = getLineForOffset((e - 1).coerceAtLeast(start))
    return (first..last).mapNotNull { line ->
        val ls = maxOf(start, getLineStart(line))
        val le = minOf(e, getLineEnd(line, visibleEnd = true))
        if (le <= ls) return@mapNotNull null
        val x0 = getHorizontalPosition(ls, usePrimaryDirection = true)
        val x1 = if (le >= getLineEnd(line, visibleEnd = true)) getLineRight(line) else getHorizontalPosition(le, usePrimaryDirection = true)
        Rect(minOf(x0, x1), getLineTop(line), maxOf(x0, x1), getLineBottom(line))
    }
}

/** 所有出现位置（不区分大小写）。 */
fun matchRanges(text: String, query: String): List<IntRange> {
    val q = query.trim()
    if (q.isEmpty()) return emptyList()
    val out = ArrayList<IntRange>()
    var i = text.indexOf(q, ignoreCase = true)
    while (i >= 0) {
        out += i until i + q.length
        i = text.indexOf(q, i + q.length, ignoreCase = true)
    }
    return out
}

/** 命中的词用 accent-soft 胶囊标出（accent-text 文字）。 */
@Composable
fun HighlightText(
    text: String,
    query: String?,
    style: TextStyle,
    color: Color,
    capsule: Color,
    highlightColor: Color,
    modifier: Modifier = Modifier,
    maxLines: Int = Int.MAX_VALUE,
) {
    val ranges = remember(text, query) { if (query == null) emptyList() else matchRanges(text, query) }
    val annotated: AnnotatedString = remember(text, ranges, highlightColor) {
        buildAnnotatedString {
            var at = 0
            ranges.forEach { r ->
                append(text.substring(at, r.first))
                withStyle(SpanStyle(color = highlightColor)) { append(text.substring(r.first, r.last + 1)) }
                at = r.last + 1
            }
            append(text.substring(at))
        }
    }
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    Text(
        annotated, style = style, color = color, maxLines = maxLines, overflow = TextOverflow.Ellipsis,
        inlineContent = emptyMap<String, InlineTextContent>(),
        onTextLayout = { layout = it },
        modifier = modifier.drawBehind {
            val l = layout ?: return@drawBehind
            val padX = 3f * density
            ranges.forEach { r ->
                l.rangeRects(r.first, r.last + 1).forEach { rect ->
                    val h = rect.height
                    val inset = h * 0.08f
                    drawRoundRect(
                        capsule,
                        topLeft = androidx.compose.ui.geometry.Offset(rect.left - padX, rect.top + inset),
                        size = androidx.compose.ui.geometry.Size(rect.width + padX * 2, h - inset * 2),
                        cornerRadius = CornerRadius(h / 2),
                    )
                }
            }
        },
    )
}
