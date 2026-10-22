package app.englishspeaker.ui.player

import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import app.englishspeaker.core.Link
import app.englishspeaker.core.SentenceSpan
import app.englishspeaker.core.WordSpan
import app.englishspeaker.ui.components.rangeRects
import app.englishspeaker.ui.theme.Ipa
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Ui

/** 一个显示单元：原文中的一个词（含其后的标点），对应时间轴里的一个 WordSpan。 */
class Token(val text: String, val index: Int, val span: WordSpan?)

/** WordSpan 的区间是 UTF-8 字节偏移，换成字符下标后切出原文（保留标点与引号）。 */
fun tokenize(s: SentenceSpan): List<Token> {
    val text = s.text
    if (s.words.isEmpty()) {
        return text.split(' ').filter { it.isNotEmpty() }.mapIndexed { i, w -> Token(w, i, null) }
    }
    val bytes = text.toByteArray(Charsets.UTF_8)
    val b2c = IntArray(bytes.size + 1)
    var bi = 0
    var ci = 0
    while (ci < text.length) {
        val cp = text.codePointAt(ci)
        val n = String(Character.toChars(cp)).toByteArray(Charsets.UTF_8).size
        for (k in 0 until n) if (bi + k < b2c.size) b2c[bi + k] = ci
        bi += n
        ci += Character.charCount(cp)
    }
    b2c[bytes.size] = text.length
    val starts = s.words.map { b2c[it.charStart.toInt().coerceIn(0, bytes.size)] }
    return s.words.mapIndexedNotNull { i, w ->
        val start = if (i == 0) 0 else starts[i]
        val end = if (i + 1 < starts.size) starts[i + 1] else text.length
        val t = text.substring(start, maxOf(start, end)).trim()
        if (t.isEmpty()) null else Token(t, i, w)
    }
}

/** 闪音 / 同化 / 省音：词下 2px violet 点线。 */
fun WordSpan.dotted(): Boolean = flap || elided || link == Link.ASSIMILATION || link == Link.FLAP

enum class LineMode { Current, Other, Pending }

/** 标注统计：N 处连读、N 个弱读。 */
fun annotationCounts(s: SentenceSpan): Pair<Int, Int> {
    val links = s.words.dropLast(1).count { it.link != null }
    val weak = s.words.count { it.weakForm != null }
    return links to weak
}

/**
 * 一句原文（UI_SPEC §3.4）。当前句：已读 t1、当前词 accent-text + accent-soft 胶囊（圆角 9，左右 6）、未读 t2；
 * 连读 ‿ violet、弱读 0.84em t3（已读 t2）600、闪音/同化/省音 2px violet 点线。
 * 非当前句 other、未合成 pending（不可点）。标注关闭时只保留逐词高亮。
 */
@Composable
fun SentenceText(
    s: SentenceSpan,
    mode: LineMode,
    currentWord: Int?,
    annotations: Boolean,
    fontSize: Float,
    lineHeight: Float,
    otherColor: Color,
    modifier: Modifier = Modifier,
    onWord: ((Int) -> Unit)? = null,
    onSentence: (() -> Unit)? = null,
    onWordLong: ((Int) -> Unit)? = null,
) {
    val p = LocalPalette.current
    val tokens = remember(s) { tokenize(s) }
    val ann = annotations && mode != LineMode.Pending
    // 夜间 650 → Android 用 SemiBold；当前句白天 700
    val currentWeight = if (p.dark) FontWeight.SemiBold else FontWeight.Bold
    val built = remember(tokens, mode, currentWord, ann, p, otherColor) {
        val ranges = IntArray(tokens.size * 2)
        val str = buildAnnotatedString {
            tokens.forEachIndexed { ti, t ->
                val w = t.span
                val weak = ann && w?.weakForm != null
                val color = when (mode) {
                    LineMode.Pending -> p.pending
                    LineMode.Other -> otherColor
                    LineMode.Current -> {
                        val cw = currentWord ?: -1
                        when {
                            t.index == cw -> p.accentText
                            t.index < cw -> if (weak) p.t2 else p.t1
                            else -> if (weak) p.t3 else p.t2
                        }
                    }
                }
                val style = if (weak) SpanStyle(color = color, fontSize = 0.84.em, fontWeight = FontWeight.SemiBold)
                else SpanStyle(color = color)
                ranges[ti * 2] = length
                withStyle(style) { append(t.text) }
                ranges[ti * 2 + 1] = length
                if (ti < tokens.size - 1) {
                    val linked = ann && w?.link != null
                    if (linked) {
                        withStyle(SpanStyle(color = if (mode == LineMode.Current) p.violet else p.violet.copy(alpha = 0.7f), fontFamily = Ipa, fontWeight = FontWeight.Bold)) {
                            append("‿")
                        }
                        // 零宽空格：连读链很长时仍可在 ‿ 之后换行
                        append("​")
                    } else append(" ")
                }
            }
        }
        str to ranges
    }
    val (text, ranges) = built
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val style = TextStyle(
        fontFamily = Ui,
        fontWeight = if (mode == LineMode.Current) currentWeight else FontWeight.SemiBold,
        fontSize = fontSize.sp,
        lineHeight = (fontSize * lineHeight).sp,
        letterSpacing = 0.003.em,
    )
    val onWordState by rememberUpdatedState(onWord)
    val onSentenceState by rememberUpdatedState(onSentence)
    val onLongState by rememberUpdatedState(onWordLong)
    fun tokenAt(offset: Int): Int? {
        for (ti in tokens.indices) if (offset >= ranges[ti * 2] && offset < ranges[ti * 2 + 1] + 1) return tokens[ti].index
        return null
    }
    Text(
        text,
        style = style,
        onTextLayout = { layout = it },
        modifier = modifier
            .pointerInput(tokens, mode) {
                detectTapGestures(
                    onTap = { pos ->
                        val l = layout ?: return@detectTapGestures
                        val w = tokenAt(l.getOffsetForPosition(pos))
                        val cb = onWordState
                        if (w != null && cb != null) cb(w) else onSentenceState?.invoke()
                    },
                    onLongPress = { pos ->
                        val l = layout ?: return@detectTapGestures
                        tokenAt(l.getOffsetForPosition(pos))?.let { onLongState?.invoke(it) }
                    },
                )
            }
            .drawBehind {
                val l = layout ?: return@drawBehind
                // 当前词胶囊
                if (mode == LineMode.Current && currentWord != null) {
                    val ti = tokens.indexOfFirst { it.index == currentWord }
                    if (ti >= 0) {
                        val padX = 6f * density
                        l.rangeRects(ranges[ti * 2], ranges[ti * 2 + 1]).forEach { r ->
                            val line = l.getLineForVerticalPosition(r.center.y)
                            val base = l.getLineBaseline(line)
                            val fs = fontSize * density * fontScale
                            val top = base - fs * 0.98f
                            val bottom = base + fs * 0.30f
                            drawRoundRect(
                                p.accentSoft, Offset(r.left - padX, top), Size(r.width + padX * 2, bottom - top),
                                CornerRadius(9f * density),
                            )
                        }
                    }
                }
                // 点线
                if (ann) {
                    val dot = 2f * density
                    tokens.forEachIndexed { ti, t ->
                        val w = t.span ?: return@forEachIndexed
                        if (!w.dotted()) return@forEachIndexed
                        l.rangeRects(ranges[ti * 2], ranges[ti * 2 + 1]).forEach { r ->
                            val line = l.getLineForVerticalPosition(r.center.y)
                            val y = l.getLineBaseline(line) + fontSize * density * fontScale * 0.22f
                            var x = r.left + dot / 2
                            // 去掉结尾标点下方的点
                            val right = r.right - trailingPunctWidth(t.text, r.width)
                            while (x < right) {
                                drawCircle(p.violet, dot / 2, Offset(x, y))
                                x += dot * 2
                            }
                        }
                    }
                }
            },
    )
}

private fun trailingPunctWidth(word: String, width: Float): Float {
    val n = word.takeLastWhile { !it.isLetterOrDigit() }.length
    return if (word.isEmpty()) 0f else width * n / word.length
}

/** 阅读设置预览用的示例句（设计稿 `CUR`）。 */
fun sampleSentence(): SentenceSpan {
    data class W(val t: String, val link: Link? = null, val weak: String? = null)
    val words = listOf(
        W("Doctors"), W("say"), W("that", weak = "ðət"), W("a", weak = "ə"), W("short"),
        W("walk", Link.CONSONANT_VOWEL), W("after"), W("each"), W("meal", Link.CONSONANT_VOWEL), W("is"),
        W("one", Link.CONSONANT_VOWEL), W("of", weak = "əv"), W("the", weak = "ðə"), W("easiest"), W("ways"),
        W("to", weak = "tə"), W("stay"), W("healthy."),
    )
    val sb = StringBuilder()
    val spans = words.mapIndexed { i, w ->
        if (i > 0) sb.append(' ')
        val start = sb.length
        sb.append(w.t)
        WordSpan(w.t, start.toUInt(), sb.length.toUInt(), (i * 300).toULong(), (i * 300 + 300).toULong(), "", w.link, w.weak, false, false)
    }
    return SentenceSpan(0u, 0u, sb.toString(), true, 0u, (words.size * 300).toULong(), spans)
}
