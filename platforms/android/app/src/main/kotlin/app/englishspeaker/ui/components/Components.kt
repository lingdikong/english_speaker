package app.englishspeaker.ui.components

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.animateFloat
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.dropShadow
import androidx.compose.ui.draw.innerShadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.shadow.Shadow
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Palette
import app.englishspeaker.ui.theme.Type
import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.HazeStyle
import dev.chrisbanes.haze.HazeTint
import dev.chrisbanes.haze.hazeEffect

/** 全局共享的背景模糊源（页面内容）。 */
val LocalHaze = compositionLocalOf<HazeState?> { null }

/**
 * 玻璃材质（UI_SPEC §2.4）：背景模糊 10 + `glass` 底色 + `glass-hi` 顶部 1px 内高光 +
 * `glass-edge` 1px 内描边 + `glass-shadow` 投影。API 31 以下 Haze 不模糊，退化为 alpha≈0.85 的 glass 色。
 */
fun Modifier.glass(shape: Shape, haze: HazeState? = null): Modifier = composed {
    val p = LocalPalette.current
    val state = haze ?: LocalHaze.current
    val solid = p.glass.copy(alpha = 0.85f)
    this
        .dropShadow(shape, Shadow(radius = 44.dp, color = p.glassShadow, offset = DpOffset(0.dp, 14.dp)))
        .clip(shape)
        .then(
            if (state != null) Modifier.hazeEffect(
                state,
                HazeStyle(
                    backgroundColor = p.bg,
                    tint = HazeTint(p.glass),
                    blurRadius = 10.dp,
                    noiseFactor = 0f,
                    fallbackTint = HazeTint(if (p.dark) Color(0xD9222734) else Color(0xD9FFFFFF)),
                ),
            ) else Modifier.background(solid)
        )
        .innerShadow(shape, Shadow(radius = 0.dp, color = p.glassHi, offset = DpOffset(0.dp, 1.dp)))
        .border(1.dp, p.glassEdge, shape)
}

/**
 * 内容在玻璃下渐隐（UI_SPEC §2.5）：从底部量起的停靠点（距离 dp → alpha），底部仍保留约 20%。
 * [stops] 由 [FadeStops] 提供；[t] 用于在两组停靠点之间插值（底部栏一行 / 两行）。
 */
fun Modifier.fadeUnder(a: List<Pair<Float, Float>>, b: List<Pair<Float, Float>> = a, t: Float = 0f, extraBottom: Dp = 0.dp): Modifier =
    this
        .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
        .drawWithContent {
            drawContent()
            val h = size.height
            if (h <= 0f) return@drawWithContent
            val extra = extraBottom.toPx()
            val stops = a.indices.map { i ->
                val d = a[i].first + (b[i].first - a[i].first) * t
                val al = a[i].second + (b[i].second - a[i].second) * t
                ((h - (d.dp.toPx() + extra)) / h).coerceIn(0f, 1f) to Color.Black.copy(alpha = al)
            }.sortedBy { it.first }
            drawRect(
                Brush.verticalGradient(*(listOf(0f to Color.Black) + stops + listOf(1f to stops.last().second)).toTypedArray()),
                blendMode = BlendMode.DstIn,
            )
        }

/**
 * 顶部渐隐遮罩：[clear] 以内完全透明（例如状态栏区域），之后在 [ramp] 内渐变到不透明。
 * 与 [fadeUnder] 一样是真正的 alpha 遮罩（DstIn），内容不会画到状态栏 / 顶部栏上。
 */
fun Modifier.fadeTop(clear: Dp, ramp: Dp): Modifier =
    this
        .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
        .drawWithContent {
            drawContent()
            val h = size.height
            if (h <= 0f) return@drawWithContent
            val c = (clear.toPx() / h).coerceIn(0f, 1f)
            val r = ((clear.toPx() + ramp.toPx()) / h).coerceIn(c, 1f)
            drawRect(
                Brush.verticalGradient(
                    0f to Color.Transparent, c to Color.Transparent,
                    // 缓入：前半段更透明，避免被截断的一行在深色底上仍清晰可见
                    (c + (r - c) * 0.6f) to Color.Black.copy(alpha = 0.35f),
                    r to Color.Black, 1f to Color.Black,
                ),
                blendMode = BlendMode.DstIn,
            )
        }

/** 设计稿 `.fade-m / .fade-m1 / .fade-mp` 的停靠点（距底部 px → 不透明度）。 */
object FadeStops {
    /** 两行（播放条 + 导航）。 */
    val m = listOf(240f to 1f, 156f to 0.62f, 26f to 0.36f, 0f to 0.2f)
    /** 一行。 */
    val m1 = listOf(170f to 1f, 86f to 0.62f, 26f to 0.36f, 0f to 0.2f)
    /** 播放页控制面板下。 */
    val mp = listOf(330f to 1f, 250f to 0.6f, 30f to 0.3f, 0f to 0.18f)
}

/** 无水波纹的点按反馈：按下时略微变淡。 */
fun Modifier.pressable(
    enabled: Boolean = true,
    onLongClick: (() -> Unit)? = null,
    role: Role? = Role.Button,
    onClick: () -> Unit,
): Modifier = composed {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val a by animateFloatAsState(if (pressed) 0.6f else 1f, tween(120), label = "press")
    this
        .graphicsLayer { alpha = a }
        .combinedClickable(
            interactionSource = source, indication = null, enabled = enabled, role = role,
            onLongClick = onLongClick, onClick = onClick,
        )
}

/** 面板：surface 底 + line 1px 内描边。 */
fun Modifier.panel(p: Palette, radius: Dp = 20.dp): Modifier {
    val s = RoundedCornerShape(radius)
    return this.background(p.surface, s).border(1.dp, p.line, s)
}

/** 卡片：card 底 + line 1px 内描边。 */
fun Modifier.card(p: Palette, radius: Dp = 16.dp): Modifier {
    val s = RoundedCornerShape(radius)
    return this.background(p.card, s).border(1.dp, p.line, s)
}

/** 当前句卡片：card 底、accent-soft 1px 内描边 + elev 投影。 */
fun Modifier.currentCard(p: Palette, radius: Dp = 22.dp): Modifier {
    val s = RoundedCornerShape(radius)
    return this
        .dropShadow(s, Shadow(radius = 30.dp, color = p.elevShadow, offset = DpOffset(0.dp, 10.dp)))
        .background(p.card, s)
        .border(1.dp, p.accentSoft, s)
}

enum class ChipKind { Accent, Violet, Neutral, Muted, Ghost, Danger }

@Composable
fun Chip(
    text: String,
    kind: ChipKind,
    modifier: Modifier = Modifier,
    height: Dp = 30.dp,
    hPad: Dp = 12.dp,
    style: TextStyle = Type.chip,
    icon: ImageVector? = null,
    iconSize: Dp = 16.dp,
    leading: (@Composable RowScope.() -> Unit)? = null,
    onClick: (() -> Unit)? = null,
) {
    val p = LocalPalette.current
    val (bg, fg) = when (kind) {
        ChipKind.Accent -> p.accentSoft to p.accentText
        ChipKind.Violet -> p.violetSoft to p.violetText
        ChipKind.Neutral -> p.control to p.t1
        ChipKind.Muted -> p.control to p.t2
        ChipKind.Ghost -> Color.Transparent to p.t2
        ChipKind.Danger -> p.dangerSoft to p.dangerText
    }
    val shape = CircleShape
    var m = modifier.height(height).clip(shape).background(bg, shape)
    if (kind == ChipKind.Ghost) m = m.border(1.dp, p.line, shape)
    if (onClick != null) m = m.pressable(onClick = onClick)
    Row(m.padding(horizontal = hPad), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally)) {
        leading?.invoke(this)
        if (icon != null) Icon(icon, null, Modifier.size(iconSize), tint = fg)
        Text(text, style = style, color = fg, maxLines = 1)
    }
}

/** 44 圆形图标按钮（可选 control 填充）。 */
@Composable
fun IconBtn(
    icon: ImageVector,
    description: String,
    modifier: Modifier = Modifier,
    size: Dp = 44.dp,
    iconSize: Dp = 22.dp,
    fill: Boolean = false,
    tint: Color = LocalPalette.current.t1,
    background: Color? = null,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    val p = LocalPalette.current
    val bg = background ?: if (fill) p.control else Color.Transparent
    Box(
        modifier
            .size(size)
            .clip(CircleShape)
            .background(bg)
            .pressable(enabled = enabled, onClick = onClick)
            .semantics { contentDescription = description },
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, null, Modifier.size(iconSize), tint = tint)
    }
}

/** 播放键：accent 圆底，accent-ink 图标。 */
@Composable
fun PlayButton(
    playing: Boolean,
    size: Dp,
    iconSize: Dp,
    modifier: Modifier = Modifier,
    buffering: Boolean = false,
    onClick: () -> Unit,
) {
    val p = LocalPalette.current
    Box(
        modifier
            .size(size)
            .clip(CircleShape)
            .background(p.accent)
            .pressable(onClick = onClick)
            .semantics { contentDescription = if (playing) "暂停" else "播放" },
        contentAlignment = Alignment.Center,
    ) {
        if (buffering) Spinner(p.accentInk, Modifier.size(iconSize))
        else Icon(if (playing) Icons.pause else Icons.play, null, Modifier.size(iconSize), tint = p.accentInk)
    }
}

/** 小转圈（Buffering）。 */
@Composable
fun Spinner(color: Color, modifier: Modifier = Modifier, stroke: Dp = 2.dp) {
    val t = androidx.compose.animation.core.rememberInfiniteTransition(label = "spin")
    val angle by t.animateFloat(
        0f, 360f,
        androidx.compose.animation.core.infiniteRepeatable(tween(900, easing = androidx.compose.animation.core.LinearEasing)),
        label = "angle",
    )
    Canvas(modifier.semantics { contentDescription = "缓冲中" }) {
        val w = stroke.toPx()
        drawArc(color.copy(alpha = 0.25f), 0f, 360f, false, topLeft = Offset(w / 2, w / 2),
            size = androidx.compose.ui.geometry.Size(size.width - w, size.height - w), style = Stroke(w))
        drawArc(color, angle, 100f, false, topLeft = Offset(w / 2, w / 2),
            size = androidx.compose.ui.geometry.Size(size.width - w, size.height - w),
            style = Stroke(w, cap = androidx.compose.ui.graphics.StrokeCap.Round))
    }
}

/** 开关 50×30：开启 accent 轨道 + accent-ink 圆点。 */
@Composable
fun Toggle(checked: Boolean, onChange: (Boolean) -> Unit, description: String) {
    val p = LocalPalette.current
    val x by animateFloatAsState(if (checked) 1f else 0f, tween(200), label = "knob")
    Box(
        Modifier
            .size(50.dp, 30.dp)
            .clip(CircleShape)
            .background(if (checked) p.accent else p.control)
            .pressable(role = Role.Switch) { onChange(!checked) }
            .semantics { contentDescription = description },
    ) {
        Box(
            Modifier
                .padding(3.dp)
                .offset(x = 20.dp * x)
                .size(24.dp)
                .clip(CircleShape)
                .background(if (checked) p.accentInk else p.t2)
        )
    }
}

/** 分段控件：容器内边距 4、圆角 14；选中项 [selectedBg] 底。 */
@Composable
fun Segmented(
    options: List<String>,
    selected: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
    containerBg: Color = LocalPalette.current.surface,
    selectedBg: Color = LocalPalette.current.control,
    bordered: Boolean = true,
    itemHeight: Dp = 40.dp,
    icons: List<ImageVector?> = emptyList(),
) {
    val p = LocalPalette.current
    val shape = RoundedCornerShape(14.dp)
    var m = modifier.fillMaxWidth().clip(shape).background(containerBg, shape)
    if (bordered) m = m.border(1.dp, p.line, shape)
    Row(m.padding(4.dp)) {
        options.forEachIndexed { i, label ->
            val sel = i == selected
            Row(
                Modifier
                    .weight(1f)
                    .height(itemHeight)
                    .clip(RoundedCornerShape(11.dp))
                    .background(if (sel) selectedBg else Color.Transparent)
                    .pressable(role = Role.Tab) { onSelect(i) },
                horizontalArrangement = Arrangement.Center,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                icons.getOrNull(i)?.let {
                    Icon(it, null, Modifier.size(16.dp), tint = if (sel) p.t1 else p.t2)
                    Spacer(Modifier.width(6.dp))
                }
                Text(label, style = Type.chip.copy(fontSize = 14.sp), color = if (sel) p.t1 else p.t2, textAlign = TextAlign.Center)
            }
        }
    }
}

/** 小标签（12/700，字距 0.08em，t2）。 */
@Composable
fun Label(text: String, modifier: Modifier = Modifier) {
    Text(text, style = Type.label, color = LocalPalette.current.t2, modifier = modifier)
}

/** 分组卡片：card 底，行高 54，行间 line 分隔。 */
@Composable
fun GroupCard(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val p = LocalPalette.current
    Column(modifier.fillMaxWidth().card(p, 16.dp).padding(horizontal = 16.dp, vertical = 4.dp), content = content)
}

@Composable
fun GroupRow(
    title: String,
    modifier: Modifier = Modifier,
    divider: Boolean = true,
    titleColor: Color = LocalPalette.current.t1,
    onClick: (() -> Unit)? = null,
    trailing: (@Composable () -> Unit)? = null,
) {
    val p = LocalPalette.current
    var m = modifier.fillMaxWidth().height(54.dp)
    if (divider) m = m.drawWithContent {
        drawContent()
        drawLine(p.line, Offset(0f, size.height - 0.5.dp.toPx()), Offset(size.width, size.height - 0.5.dp.toPx()), 1.dp.toPx())
    }
    if (onClick != null) m = m.clickable(interactionSource = null, indication = null, onClick = onClick)
    Row(m, verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = Type.body, color = titleColor, modifier = Modifier.weight(1f))
        trailing?.invoke()
    }
}

/** 4px 进度条（继续收听卡片）。 */
@Composable
fun ProgressLine(fraction: Float, modifier: Modifier = Modifier, height: Dp = 4.dp) {
    val p = LocalPalette.current
    Canvas(modifier.fillMaxWidth().height(height)) {
        val r = androidx.compose.ui.geometry.CornerRadius(size.height / 2)
        drawRoundRect(p.waveE, cornerRadius = r)
        val w = size.width * fraction.coerceIn(0f, 1f)
        if (w > 0f) drawRoundRect(p.accent, size = androidx.compose.ui.geometry.Size(w, size.height), cornerRadius = r)
    }
}

/** 品牌图形：圆角方块内 5 根竖条声波。 */
@Composable
fun BrandMark(size: Dp = 56.dp) {
    val p = LocalPalette.current
    val scale = size.value / 56f
    Row(
        Modifier.size(size).clip(RoundedCornerShape(16.dp * scale)).background(p.accent),
        horizontalArrangement = Arrangement.spacedBy(3.dp * scale, Alignment.CenterHorizontally),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        listOf(14, 26, 36, 22, 12).forEach { h ->
            Box(Modifier.size(4.dp * scale, h.dp * scale).clip(RoundedCornerShape(2.dp)).background(p.accentInk))
        }
    }
}

/** 页面大标题行。 */
@Composable
fun LargeTitle(text: String, modifier: Modifier = Modifier, trailing: (@Composable RowScope.() -> Unit)? = null) {
    val p = LocalPalette.current
    Row(modifier.fillMaxWidth().height(44.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(text, style = Type.largeTitle, color = p.t1, modifier = Modifier.weight(1f))
        if (trailing != null) Row(horizontalArrangement = Arrangement.spacedBy(8.dp), content = trailing)
    }
}

/** Sheet 顶部拖动条（40×5，control）。 */
@Composable
fun SheetHandle() {
    val p = LocalPalette.current
    Box(Modifier.fillMaxWidth().padding(top = 12.dp), contentAlignment = Alignment.Center) {
        Box(Modifier.size(40.dp, 5.dp).clip(RoundedCornerShape(3.dp)).background(p.control))
    }
}

/** 粗体文本样式辅助。 */
fun TextStyle.bold() = copy(fontWeight = FontWeight.Bold)
