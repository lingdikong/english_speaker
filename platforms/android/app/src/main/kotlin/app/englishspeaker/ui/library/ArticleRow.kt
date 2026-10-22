package app.englishspeaker.ui.library

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.englishspeaker.core.ArticleSummary
import app.englishspeaker.core.AudioStatus
import app.englishspeaker.data.SpeakerRepository
import app.englishspeaker.ui.components.Chip
import app.englishspeaker.ui.components.ChipKind
import app.englishspeaker.ui.components.IconBtn
import app.englishspeaker.ui.components.HighlightText
import app.englishspeaker.ui.components.WaveCover
import app.englishspeaker.ui.components.pressable
import app.englishspeaker.ui.components.panel
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Type
import kotlin.math.roundToInt

/** 时长文案：已定稿 “6 分钟”，未定稿 “约 6 分钟”。 */
fun durationText(a: ArticleSummary): String {
    val ms = a.durationMs.toLong().takeIf { it > 0 } ?: (a.totalSentences.toLong() * 3500L)
    val min = maxOf(1, (ms / 60000.0).roundToInt())
    return if (a.status == AudioStatus.READY) "$min 分钟" else "约 $min 分钟"
}

/** 行副标题：“时长 · 音色”（未转换时只有时长）。 */
fun metaText(repo: SpeakerRepository, a: ArticleSummary): String =
    if (a.status == AudioStatus.NOT_CACHED) durationText(a)
    else "${durationText(a)} · ${repo.voiceLabel(a.voiceId, " ")}"

/** 合成状态 chip（UI_SPEC §3.3，替代旧版下载图标）。 */
@Composable
fun StatusChip(repo: SpeakerRepository, a: ArticleSummary) {
    val p = LocalPalette.current
    val small = Type.chip.copy(fontSize = 12.sp)
    when (a.status) {
        AudioStatus.NOT_CACHED -> Chip("未转换", ChipKind.Ghost, height = 22.dp, hPad = 8.dp, style = small, onClick = { repo.synthesize(a.id) })
        AudioStatus.QUEUED -> Chip("排队中", ChipKind.Accent, height = 22.dp, hPad = 8.dp, style = small)
        AudioStatus.SYNTHESIZING, AudioStatus.PARTIAL -> {
            val total = a.totalSentences.toInt().coerceAtLeast(1)
            val f = a.readySentences.toFloat() / total
            Chip("${a.readySentences}/${a.totalSentences}", ChipKind.Accent, height = 22.dp, hPad = 8.dp, style = small, leading = {
                Box(Modifier.size(22.dp, 3.dp).clip(RoundedCornerShape(2.dp)).background(p.waveE)) {
                    Box(Modifier.size(22.dp * f, 3.dp).clip(RoundedCornerShape(2.dp)).background(p.accent))
                }
            })
        }
        AudioStatus.READY -> if (a.pinned) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                Icon(Icons.saved, null, Modifier.size(12.dp), tint = p.accent)
                Text("已保存", style = small, color = p.accentText)
            }
        }
        AudioStatus.FAILED -> Chip("失败 · 重试", ChipKind.Danger, height = 22.dp, hPad = 8.dp, style = small, onClick = { repo.synthesize(a.id) })
    }
}

/**
 * 资料库行：56 封面 · 标题 15/700 · “时长 · 音色” + 状态 chip · 更多（⋯）。
 * 长按或 ⋯：保存 / 取消保存、换音色、删除缓存、删除文章。
 */
@Composable
fun ArticleRow(
    repo: SpeakerRepository,
    a: ArticleSummary,
    index: Int,
    onClick: () -> Unit,
    query: String? = null,
    snippet: String? = null,
) {
    val p = LocalPalette.current
    var menu by remember { mutableStateOf(false) }
    Row(
        Modifier
            .fillMaxWidth()
            .drawWithContent {
                drawContent()
                val y = size.height - 0.5.dp.toPx()
                drawLine(p.line, Offset(0f, y), Offset(size.width, y), 1.dp.toPx())
            }
            .pressable(onLongClick = { menu = true }, onClick = onClick)
            .padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        WaveCover(a.title, 56, if (index % 2 == 1) p.accent else p.violet)
        Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            HighlightText(a.title, query, Type.rowTitle, p.t1, p.accentSoft, p.accentText, maxLines = 2)
            if (snippet != null) {
                HighlightText(snippet, query, Type.secondary, p.t2, p.accentSoft, p.accentText, Modifier.padding(top = 4.dp), maxLines = 2)
            }
            Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(metaText(repo, a), style = Type.secondary, color = p.t2, maxLines = 1)
                StatusChip(repo, a)
            }
        }
        Box {
            IconBtn(Icons.more, "更多操作", tint = p.t2) { menu = true }
            RowMenu(repo, a, menu) { menu = false }
        }
    }
}

@Composable
private fun RowMenu(repo: SpeakerRepository, a: ArticleSummary, expanded: Boolean, onDismiss: () -> Unit) {
    val p = LocalPalette.current
    var voices by remember { mutableStateOf(false) }
    val list by repo.voices.collectAsState()
    DropdownMenu(
        expanded = expanded,
        onDismissRequest = { voices = false; onDismiss() },
        containerColor = p.surface,
        shape = RoundedCornerShape(16.dp),
    ) {
        @Composable
        fun item(text: String, color: androidx.compose.ui.graphics.Color = p.t1, action: () -> Unit) =
            DropdownMenuItem(text = { Text(text, style = Type.body, color = color) }, onClick = action)
        if (!voices) {
            item(if (a.pinned) "取消保存" else "保存（离线保留）") { repo.setPinned(a.id, !a.pinned); onDismiss() }
            item("换音色") { voices = true }
            if (a.cachedBytes > 0u) item("删除缓存") { repo.removeDownload(a.id); onDismiss() }
            HorizontalDivider(color = p.line)
            item("删除文章", p.dangerText) { repo.delete(a.id); onDismiss() }
        } else {
            list.forEach { v ->
                val sel = v.id == (a.voiceId ?: repo.defaultVoice.value)
                item(repo.voiceLabel(v.id), if (sel) p.accentText else p.t1) {
                    repo.synthesize(a.id, v.id); voices = false; onDismiss()
                }
            }
        }
    }
}

/** “继续收听”卡片（surface 面板，72 封面 + 标题 18 + 进度说明 + 48 播放键 + 4px 进度条）。 */
@Composable
fun ContinueCard(
    title: String,
    detail: String,
    progress: Float,
    playing: Boolean,
    buffering: Boolean,
    onPlay: () -> Unit,
    onOpen: () -> Unit,
) {
    val p = LocalPalette.current
    Column(
        Modifier
            .fillMaxWidth()
            .panel(p, 22.dp)
            .clip(RoundedCornerShape(22.dp))
            .pressable(onClick = onOpen)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        app.englishspeaker.ui.components.Label("继续收听")
        Row(verticalAlignment = Alignment.CenterVertically) {
            WaveCover(title, 72, p.accent)
            Spacer(Modifier.width(14.dp))
            Column(Modifier.weight(1f)) {
                Text(title, style = Type.displayTitle, color = p.t1, maxLines = 2, overflow = TextOverflow.Ellipsis)
                Text(detail, style = Type.secondary, color = p.t2, modifier = Modifier.padding(top = 4.dp))
            }
            Spacer(Modifier.width(8.dp))
            app.englishspeaker.ui.components.PlayButton(playing, 48.dp, 20.dp, buffering = buffering, onClick = onPlay)
        }
        app.englishspeaker.ui.components.ProgressLine(progress)
    }
}

/** 空状态：品牌图形 + “导入第一篇文章”。 */
@Composable
fun EmptyState(onImport: () -> Unit) {
    val p = LocalPalette.current
    Column(
        Modifier.fillMaxWidth().padding(top = 80.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        app.englishspeaker.ui.components.BrandMark()
        Text("还没有文章", style = Type.section, color = p.t1)
        Text("粘贴一段英文，离线转换成可以边听边读的音频。", style = Type.secondary, color = p.t2)
        Box(
            Modifier.height(48.dp).clip(RoundedCornerShape(999.dp)).background(p.accent)
                .pressable(onClick = onImport).padding(horizontal = 22.dp),
            contentAlignment = Alignment.Center,
        ) { Text("导入第一篇文章", style = Type.rowTitle, color = p.accentInk) }
    }
}
