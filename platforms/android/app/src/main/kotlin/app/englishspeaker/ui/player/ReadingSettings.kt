package app.englishspeaker.ui.player

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.border
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.dropShadow
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.shadow.Shadow
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.englishspeaker.data.ThemeMode
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.components.AppSheet
import app.englishspeaker.ui.components.Chip
import app.englishspeaker.ui.components.ChipKind
import app.englishspeaker.ui.components.GroupCard
import app.englishspeaker.ui.components.GroupRow
import app.englishspeaker.ui.components.Label
import app.englishspeaker.ui.components.Segmented
import app.englishspeaker.ui.components.SheetHeader
import app.englishspeaker.ui.components.Toggle
import app.englishspeaker.ui.components.card
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Type

/** 播放页里的“阅读设置” Sheet（与设置 Tab 是同一组设置）。 */
@Composable
fun ReadingSettingsSheet(vm: AppViewModel, onDismiss: () -> Unit) {
    AppSheet(onDismiss) { close ->
        Column(
            Modifier
                .verticalScroll(rememberScrollState())
                .padding(start = 20.dp, end = 20.dp, top = 12.dp, bottom = 24.dp)
                .navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(22.dp),
        ) {
            SheetHeader("阅读设置", close)
            ReadingSettingsContent(vm, showTheme = true)
        }
    }
}

private fun otherLabel(a: Float) = when {
    a < 0.55f -> "较暗 50%"
    a < 0.65f -> "适中 60%"
    else -> "较亮 70%"
}

private fun sizeLabel(s: Int) = when {
    s <= 21 -> "较小"
    s <= 23 -> "偏小"
    s == 24 -> "标准"
    s <= 27 -> "较大"
    else -> "特大"
}

/** 预览 · 主题 · 字号 · 行距 · 非当前句亮度 · 专注模式 · 跟读标注。 */
@Composable
fun ReadingSettingsContent(vm: AppViewModel, showTheme: Boolean) {
    val p = LocalPalette.current
    val st = vm.settings
    val fontSize by st.fontSize.collectAsState()
    val lineHeight by st.lineHeight.collectAsState()
    val otherAlpha by st.otherAlpha.collectAsState()
    val focus by st.focusMode.collectAsState()
    val theme by st.theme.collectAsState()
    val annotations by vm.repo.showAnnotations.collectAsState()
    val sample = remember { sampleSentence() }

    Column(Modifier.fillMaxWidth().card(p, 20.dp).padding(horizontal = 18.dp, vertical = 16.dp)) {
        Label("预览", Modifier.padding(bottom = 10.dp))
        SentenceText(sample, LineMode.Current, 5, annotations, fontSize.toFloat(), lineHeight, p.otherAt(otherAlpha))
    }
    if (showTheme) ThemeSegment(theme) { st.setTheme(it) }
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Label("字号", Modifier.weight(1f))
            Text("${sizeLabel(fontSize)} · ${fontSize}px", style = Type.secondary, color = p.t2)
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            Text("A", style = Type.rowTitle.copy(fontSize = 14.sp), color = p.t1)
            FontSlider(fontSize, Modifier.weight(1f)) { st.setFontSize(it) }
            Text("A", style = Type.rowTitle.copy(fontSize = 22.sp), color = p.t1)
        }
    }
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Label("行距")
        Segmented(listOf("舒适 1.6", "紧凑 1.4"), if (lineHeight > 1.5f) 0 else 1, { st.setLineHeight(if (it == 0) 1.6f else 1.4f) })
    }
    GroupCard {
        GroupRow("非当前句亮度") {
            Chip("${otherLabel(otherAlpha)} ›", ChipKind.Neutral) {
                st.setOtherAlpha(when { otherAlpha < 0.55f -> 0.6f; otherAlpha < 0.65f -> 0.7f; else -> 0.5f })
            }
        }
        GroupRow("专注模式（5 秒后收起控件）") { Toggle(focus, { st.setFocusMode(it) }, "专注模式") }
        GroupRow("显示跟读标注", divider = false) { Toggle(annotations, { vm.repo.setAnnotations(it) }, "显示跟读标注") }
    }
}

@Composable
fun ThemeSegment(theme: ThemeMode, onSelect: (ThemeMode) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Label("主题")
        Segmented(
            listOf("跟随系统", "白天", "夜间"),
            when (theme) { ThemeMode.System -> 0; ThemeMode.Light -> 1; ThemeMode.Dark -> 2 },
            { onSelect(listOf(ThemeMode.System, ThemeMode.Light, ThemeMode.Dark)[it]) },
            icons = listOf(null, Icons.line("sun"), Icons.line("moon")),
        )
    }
}

/** 字号滑杆：20–30，轨道 6（wave-e / accent），24 圆形滑块（card 底 + 投影）。 */
@Composable
private fun FontSlider(value: Int, modifier: Modifier = Modifier, onChange: (Int) -> Unit) {
    val p = LocalPalette.current
    val f = (value - 20) / 10f
    fun pick(x: Float, w: Float) = (20 + Math.round(((x / w).coerceIn(0f, 1f)) * 10)).coerceIn(20, 30)
    Box(
        modifier
            .height(30.dp)
            .semantics { contentDescription = "字号 $value" }
            .pointerInput(Unit) { detectTapGestures { onChange(pick(it.x, size.width.toFloat())) } }
            .pointerInput(Unit) { detectHorizontalDragGestures { ch, _ -> onChange(pick(ch.position.x, size.width.toFloat())) } },
        contentAlignment = Alignment.CenterStart,
    ) {
        Canvas(Modifier.fillMaxWidth().height(6.dp)) {
            drawRoundRect(p.waveE, cornerRadius = CornerRadius(3.dp.toPx()))
            drawRoundRect(p.accent, size = Size(size.width * f, size.height), cornerRadius = CornerRadius(3.dp.toPx()))
        }
        androidx.compose.foundation.layout.BoxWithConstraints(Modifier.fillMaxWidth()) {
            val x = (maxWidth - 24.dp) * f
            Box(
                Modifier
                    .padding(start = x)
                    .size(24.dp)
                    .dropShadow(CircleShape, Shadow(4.dp, Color.Black.copy(alpha = 0.3f), offset = DpOffset(0.dp, 1.dp)))
                    .background(p.card, CircleShape)
                    .border(1.dp, p.line, CircleShape)
            )
        }
    }
}

