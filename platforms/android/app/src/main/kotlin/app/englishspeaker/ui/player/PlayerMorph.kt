package app.englishspeaker.ui.player

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.AnimationVector1D
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.Bar
import app.englishspeaker.ui.BarState
import app.englishspeaker.ui.theme.LocalReduceMotion
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

/** 手指下拉多少距离走完 100% 收起进度。 */
private val dismissDistance = 460.dp
/** 顶部多高的区域内按下才算收起手势；其余区域的下滑归歌词滚动。 */
private val gripHeight = 220.dp
/** 走过这个进度就判定“收起”（否则弹回全屏）。 */
private const val dismissThreshold = 0.30f

/**
 * 播放页的收起/展开容器，行为对齐 iOS 的 PlayerView：
 *
 * 整页始终按全屏尺寸排版，再放进一个逐渐缩小、圆角逐渐变大的“取景窗”里，
 * 终点是底部播放条左端那个圆形封面 —— 点小球进入即反向播放同一条曲线。
 *
 * 缩放锚在**顶部**：被裁掉的是底部控制面板，顶部（手势所在的头部）始终在窗口内，
 * 否则手势会被裁掉而中断。
 */
@Composable
fun PlayerMorph(
    vm: AppViewModel,
    collapse: Animatable<Float, AnimationVector1D>,
    barState: BarState,
    base: Dp,
    ballSize: Dp,
    onDismiss: () -> Unit,
    content: @Composable () -> Unit,
) {
    val reduce = LocalReduceMotion.current
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    var dragging by remember { mutableStateOf(false) }
    var dismissing by remember { mutableStateOf(false) }
    val c by collapse.asState()

    // 展开：从底部小球弹到全屏
    LaunchedEffect(Unit) {
        collapse.snapTo(1f)
        collapse.animateTo(
            0f,
            // 临界阻尼：欠阻尼弹簧会冲过 0 → 负进度 → 窗口比屏幕还大、圆角为负 → 到位的瞬间“跳”一下
            if (reduce) tween(0) else spring(dampingRatio = 1f, stiffness = 260f),
        )
    }
    // 返回键 / “收起”按钮：走同一条收起曲线
    LaunchedEffect(vm.collapseRequest) {
        if (vm.collapseRequest == 0 || dismissing) return@LaunchedEffect
        dismissing = true
        collapse.animateTo(1f, if (reduce) tween(0) else tween(260))
        onDismiss()
    }

    BoxWithConstraints(
        Modifier
            .fillMaxSize()
            // 手势必须挂在**未变换**的外层：挂在被缩放的层上，手指位移会被自身缩放反复放大，
            // 形成正反馈（位移↑→进度↑→缩放↓→位移↑），慢拖时会在几个解之间跳 → 抖动。
            .pointerInput(Unit) {
                val grip = with(density) { gripHeight.toPx() }
                val dist = with(density) { dismissDistance.toPx() }
                detectDragGestures(
                    onDragStart = { off -> dragging = !dismissing && off.y < grip },
                    onDragEnd = {
                        if (dragging) {
                            dragging = false
                            if (collapse.value > dismissThreshold) {
                                dismissing = true
                                scope.launch {
                                    collapse.animateTo(1f, if (reduce) tween(0) else tween(240))
                                    onDismiss()
                                }
                            } else {
                                scope.launch {
                                    collapse.animateTo(0f, spring(dampingRatio = 1f, stiffness = 300f))
                                }
                            }
                        }
                    },
                    onDragCancel = { dragging = false },
                    onDrag = { _, amount ->
                        if (dragging) {
                            val next = (collapse.value + amount.y / dist).coerceIn(0f, 1f)
                            scope.launch { collapse.snapTo(next) }
                        }
                    },
                )
            },
    ) {
        val boxW = maxWidth
        val boxH = maxHeight
        val w = with(density) { boxW.toPx() }
        val h = with(density) { boxH.toPx() }
        val b = with(density) { ballSize.toPx() }
        // 进度必须夹到 0…1：弹簧冲过头会产生负进度，算出比屏幕还大的窗口和负圆角
        val cc = c.coerceIn(0f, 1f)
        // 尺寸走二次曲线：开头几乎只跟着手下移，末段才快速收成球
        // （尺寸也线性时，手指走 460 卡片顶边要跑一千多像素 → 感觉是“飞”出去而不是跟手）
        val sp = cc * cc
        val cw = w + (b - w) * sp
        val ch = h + (b - h) * sp
        // 位置线性插值到球心
        val ball = remember(barState, base, boxW, boxH, density) {
            val left = when (barState) {
                BarState.A -> Bar.side
                BarState.B -> Bar.side + Bar.height + Bar.gap
                BarState.C -> boxW - Bar.side - Bar.height
            }
            val bottom = if (barState == BarState.A) base + Bar.height + Bar.gap else base
            Offset(
                with(density) { (left + Bar.height / 2).toPx() },
                with(density) { (boxH - bottom - Bar.height / 2).toPx() },
            )
        }
        val cx = w / 2 + (ball.x - w / 2) * cc
        val cy = h / 2 + (ball.y - h / 2) * cc
        val radius = (b / 2) * cc
        // 最后一程淡出，让卡片化进底部播放条（播放条本来就在下面，不会闪）
        val alpha = if (cc > 0.82f) ((1f - cc) / 0.18f).coerceIn(0f, 1f) else 1f

        Box(
            Modifier
                .offset { IntOffset((cx - cw / 2).roundToInt(), (cy - ch / 2).roundToInt()) }
                .size(with(density) { cw.toDp() }, with(density) { ch.toDp() })
                .graphicsLayer { this.alpha = alpha }
                .clip(RoundedCornerShape(with(density) { radius.toDp() })),
            contentAlignment = Alignment.TopCenter,
        ) {
            Box(
                Modifier
                    .requiredSize(boxW, boxH)
                    .graphicsLayer {
                        scaleX = (cw / w).coerceAtLeast(0.001f)
                        scaleY = scaleX
                        transformOrigin = TransformOrigin(0.5f, 0f)
                    },
            ) { content() }
        }
    }
}
