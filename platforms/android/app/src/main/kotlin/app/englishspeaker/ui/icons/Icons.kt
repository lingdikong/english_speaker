package app.englishspeaker.ui.icons

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.unit.dp

/**
 * 图标：直接由设计稿 `docs/design/visual-v2.html` 中 `P`（线性，24 网格）与
 * `PLAY / PAUSE / MORE / SAVED` 的 SVG 路径构建。颜色用 Icon 的 tint（路径本身用黑色占位）。
 */
object Icons {
    private fun circle(cx: Float, cy: Float, r: Float) =
        "M${cx - r} ${cy}a$r $r 0 1 0 ${2 * r} 0a$r $r 0 1 0 ${-2 * r} 0z"

    private fun rrect(x: Float, y: Float, w: Float, h: Float, r: Float) =
        "M${x + r} ${y}h${w - 2 * r}a$r $r 0 0 1 $r ${r}v${h - 2 * r}a$r $r 0 0 1 ${-r} ${r}h${-(w - 2 * r)}a$r $r 0 0 1 ${-r} ${-r}v${-(h - 2 * r)}a$r $r 0 0 1 $r ${-r}z"

    /** 线性图标路径（设计稿 `P`）。 */
    private val P: Map<String, List<String>> = mapOf(
        "home" to listOf("M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z"),
        "lib" to listOf("M5 4h4v16H5zM11 4h4v16h-4z", "m17 5 3.5.8-3 14.6-3.5-.8"),
        "search" to listOf(circle(11f, 11f, 7f), "m20 20-3.5-3.5"),
        "plus" to listOf("M12 5v14M5 12h14"),
        "prev" to listOf("M5 5v14", "M19 6v12l-9-6z"),
        "next" to listOf("M19 5v14", "M5 6v12l9-6z"),
        "loop" to listOf("M17 3l3 3-3 3", "M4 11V9a3 3 0 0 1 3-3h13", "M7 21l-3-3 3-3", "M20 13v2a3 3 0 0 1-3 3H4", "M11.5 10.5 13 9.5v5"),
        "queue" to listOf("M4 6h11M4 12h11M4 18h7M18 14v6M15 17h6"),
        "down" to listOf("m6 9 6 6 6-6"),
        "close" to listOf("M6 6l12 12M18 6 6 18"),
        "annot" to listOf("M4 8c2.5 4 6 4 8 0s5.5-4 8 0"),
        "vol" to listOf("M11 5 6 9H3v6h3l5 4z", "M15.5 8.5a5 5 0 0 1 0 7"),
        "textsize" to listOf("M4 18 9 6l5 12M5.8 14h6.4", "M15 18l3-7 3 7M16 16h4"),
        "sun" to listOf(circle(12f, 12f, 4f), "M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"),
        "gear" to listOf(
            circle(12f, 12f, 3f),
            "M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
        ),
        "wave" to listOf("M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2"),
        "bookmark" to listOf("M6 3.5h12v17l-6-4-6 4z"),
        // 拖动排序把手（设计稿未给出，按同一线性风格补充）
        "grip" to listOf("M5 9h14M5 15h14"),
        "moon" to listOf("M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z"),
        // 翻译与查词
        "star" to listOf("m12 3.5 2.6 5.3 5.9.9-4.3 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z"),
        "translate" to listOf("M3 5h9M7.5 3v2M5 9c1.2 2.8 3.4 4.8 6 6M10 5c-.8 4-3.3 7.4-7 9.5", "m13 21 4-10 4 10M14.4 17.5h5.2"),
        "sparkle" to listOf("M12 3c.6 4.6 2.4 6.4 7 7-4.6.6-6.4 2.4-7 7-.6-4.6-2.4-6.4-7-7 4.6-.6 6.4-2.4 7-7z"),
        "trash" to listOf("M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3"),
    )

    private val cache = HashMap<String, ImageVector>()

    /** 线性图标：stroke 默认 1.75（选中 2.0）。 */
    fun line(key: String, stroke: Float = 1.75f): ImageVector = synchronized(cache) {
        cache.getOrPut("$key@$stroke") {
            val b = ImageVector.Builder(key, 24.dp, 24.dp, 24f, 24f)
            P.getValue(key).forEach { d ->
                b.addPath(
                    PathParser().parsePathString(d).toNodes(),
                    stroke = SolidColor(Color.Black), strokeLineWidth = stroke,
                    strokeLineCap = StrokeCap.Round, strokeLineJoin = StrokeJoin.Round,
                )
            }
            b.build()
        }
    }

    private fun filled(name: String, vararg paths: String): ImageVector {
        val b = ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f)
        paths.forEach { b.addPath(PathParser().parsePathString(it).toNodes(), fill = SolidColor(Color.Black)) }
        return b.build()
    }

    val play: ImageVector by lazy { filled("play", "M7 4.5v15l12-7.5z") }
    val pause: ImageVector by lazy { filled("pause", rrect(6f, 5f, 4f, 14f, 1.2f), rrect(14f, 5f, 4f, 14f, 1.2f)) }
    val more: ImageVector by lazy { filled("more", circle(5f, 12f, 1.8f), circle(12f, 12f, 1.8f), circle(19f, 12f, 1.8f)) }
    val saved: ImageVector by lazy { filled("saved", "M6 3.5h12v17l-6-4-6 4z") }

    val home get() = line("home")
    val lib get() = line("lib")
    val search get() = line("search")
    val gear get() = line("gear")
}
