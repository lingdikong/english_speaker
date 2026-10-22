package app.englishspeaker.ui

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.englishspeaker.core.ArticleSummary
import app.englishspeaker.core.PlayerStatus
import app.englishspeaker.ui.components.IconBtn
import app.englishspeaker.ui.components.Spinner
import app.englishspeaker.ui.components.Wave
import app.englishspeaker.ui.components.drawBars
import app.englishspeaker.ui.components.glass
import app.englishspeaker.ui.components.pressable
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.LocalReduceMotion
import app.englishspeaker.ui.theme.Type
import app.englishspeaker.ui.theme.fadeSpec
import app.englishspeaker.ui.theme.layoutSpring

/** 底部栏尺寸（UI_SPEC §4.2）。 */
object Bar {
    val height = 60.dp
    val side = 16.dp
    val gap = 10.dp
    /** 底部离手势条 26。 */
    val bottom = 26.dp
}

private val tabs = listOf(
    Triple(Tab.Home, "首页", "home"),
    Triple(Tab.Library, "资料库", "lib"),
    Triple(Tab.Search, "搜索", "search"),
    Triple(Tab.Settings, "设置", "gear"),
)

/** 播放条数据。 */
class MiniInfo(
    val article: ArticleSummary,
    val status: PlayerStatus,
    val progress: Float,
    val line: String,
)

/**
 * 两个玻璃药丸：播放条（有文章时）和导航。三态：
 * A 两行全宽；B 导航收成左侧 60×60 小球、播放条 left 86；C 导航展开 right 86、播放条收成右侧小球。
 */
@Composable
fun BottomBar(
    state: BarState,
    tab: Tab,
    mini: MiniInfo?,
    base: Dp,
    onTab: (Tab) -> Unit,
    onExpandTabs: () -> Unit,
    onExpandPlayer: () -> Unit,
    onOpenPlayer: () -> Unit,
    onToggle: () -> Unit,
    onNext: () -> Unit,
) {
    val reduce = LocalReduceMotion.current
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val w = maxWidth
        val h = maxHeight
        val hasPlayer = mini != null
        // 导航
        val tabsRight = when (state) {
            BarState.A -> Bar.side
            BarState.B -> w - Bar.side - Bar.height
            BarState.C -> if (hasPlayer) Bar.side + Bar.height + Bar.gap else Bar.side
        }
        val tr by animateDpAsState(tabsRight, layoutSpring(reduce), label = "tabsRight")
        val tabsFull by animateFloatAsState(if (state == BarState.B) 0f else 1f, fadeSpec(reduce, 220), label = "tabsFull")
        Box(
            Modifier
                .offset(x = Bar.side, y = h - base - Bar.height)
                .size(w - Bar.side - tr, Bar.height)
                .glass(CircleShape)
        ) {
            if (tabsFull > 0.01f) {
                Row(
                    Modifier.fillMaxSize().graphicsLayer { alpha = tabsFull }.padding(horizontal = 6.dp),
                ) {
                    tabs.forEach { (t, label, icon) ->
                        val on = t == tab
                        val p = LocalPalette.current
                        Column(
                            Modifier
                                .weight(1f)
                                .fillMaxHeight()
                                .padding(vertical = 6.dp, horizontal = 1.dp)
                                .clip(CircleShape)
                                .background(if (on) p.accentSoft else androidx.compose.ui.graphics.Color.Transparent)
                                .pressable(enabled = state != BarState.B, role = Role.Tab) { onTab(t) }
                                .semantics { selected = on },
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.spacedBy(3.dp, Alignment.CenterVertically),
                        ) {
                            Icon(Icons.line(icon, if (on) 2f else 1.75f), null, Modifier.size(22.dp), tint = if (on) p.accentText else p.t2)
                            Text(label, style = Type.tab, color = if (on) p.accentText else p.t2, maxLines = 1, softWrap = false)
                        }
                    }
                }
            }
            if (tabsFull < 0.99f) {
                val p = LocalPalette.current
                val icon = tabs.first { it.first == tab }
                Box(
                    Modifier
                        .size(Bar.height)
                        .graphicsLayer { alpha = 1f - tabsFull }
                        .clip(CircleShape)
                        .pressable(enabled = state == BarState.B, onClick = onExpandTabs)
                        .semantics { contentDescription = "展开导航（当前：${icon.second}）" },
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(Icons.line(icon.third, 2f), null, Modifier.size(24.dp), tint = p.accentText)
                }
            }
        }

        // 播放条
        if (mini != null) {
            val left = when (state) {
                BarState.A -> Bar.side
                BarState.B -> Bar.side + Bar.height + Bar.gap
                BarState.C -> w - Bar.side - Bar.height
            }
            val bottom = if (state == BarState.A) base + Bar.height + Bar.gap else base
            val l by animateDpAsState(left, layoutSpring(reduce), label = "playerLeft")
            val b by animateDpAsState(bottom, layoutSpring(reduce), label = "playerBottom")
            val rest by animateFloatAsState(if (state == BarState.C) 0f else 1f, fadeSpec(reduce), label = "rest")
            Box(
                Modifier
                    .offset(x = l, y = h - b - Bar.height)
                    .size(w - Bar.side - l, Bar.height)
                    .glass(CircleShape)
                    .pressable(enabled = state != BarState.C, onClick = onOpenPlayer)
                    .semantics { contentDescription = "正在播放：${mini.article.title}" }
            ) {
                MiniContent(mini, rest, onToggle, onNext)
                if (state == BarState.C) {
                    Box(
                        Modifier.fillMaxSize().clip(CircleShape)
                            .pressable(onClick = onExpandPlayer)
                            .semantics { contentDescription = "展开播放条" }
                    )
                }
            }
        }
    }
}

@Composable
private fun MiniContent(mini: MiniInfo, restAlpha: Float, onToggle: () -> Unit, onNext: () -> Unit) {
    val p = LocalPalette.current
    Row(
        Modifier.fillMaxSize().padding(start = 7.dp, end = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(11.dp),
    ) {
        RingCover(mini.article.title, mini.progress)
        if (restAlpha > 0.01f) {
            Row(
                Modifier.weight(1f).graphicsLayer { alpha = restAlpha },
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                Column(Modifier.weight(1f)) {
                    Text(mini.article.title, style = Type.rowTitle.copy(fontSize = 14.sp), color = p.t1, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(mini.line, style = Type.secondary.copy(fontSize = 12.sp, lineHeight = 16.sp), color = p.accentText, maxLines = 1, overflow = TextOverflow.Ellipsis)
                }
                val playing = mini.status == PlayerStatus.PLAYING
                if (mini.status == PlayerStatus.BUFFERING) {
                    Box(Modifier.size(44.dp), contentAlignment = Alignment.Center) { Spinner(p.t1, Modifier.size(20.dp)) }
                } else {
                    IconBtn(if (playing) Icons.pause else Icons.play, if (playing) "暂停" else "播放", onClick = onToggle)
                }
                IconBtn(Icons.line("next", 1.9f), "下一句", onClick = onNext)
            }
        }
    }
}

/** 46 圆形封面（card 底，内部 5 根 accent 声波）+ 2.5 进度环（槽 wave-e，进度 accent，从 12 点顺时针）。 */
@Composable
fun RingCover(title: String, progress: Float) {
    val p = LocalPalette.current
    val hs = remember(title) { Wave.bars(Wave.seedOf(title), 5, 4, 18) }
    Box(Modifier.size(46.dp), contentAlignment = Alignment.Center) {
        Canvas(Modifier.size(46.dp)) {
            val sw = 2.5.dp.toPx()
            val r = 21.5.dp.toPx()
            val c = Offset(size.width / 2, size.height / 2)
            drawCircle(p.waveE, r, c, style = Stroke(sw))
            if (progress > 0f) {
                drawArc(
                    p.accent, -90f, 360f * progress.coerceIn(0f, 1f), false,
                    topLeft = Offset(c.x - r, c.y - r), size = Size(r * 2, r * 2),
                    style = Stroke(sw, cap = StrokeCap.Round),
                )
            }
        }
        Box(Modifier.size(36.dp).clip(CircleShape).background(p.card), contentAlignment = Alignment.Center) {
            Canvas(Modifier.width(36.dp).height(18.dp)) {
                drawBars(hs, 18, 8.dp.toPx(), 2.dp.toPx(), 2.dp.toPx(), bottom = false) { p.accent }
            }
        }
    }
}

/** 列表底部留白：保证最后一行能滚到玻璃之上。 */
fun barContentPadding(hasPlayer: Boolean, base: Dp): Dp =
    base + Bar.height + (if (hasPlayer) Bar.height + Bar.gap else 0.dp) + 24.dp

