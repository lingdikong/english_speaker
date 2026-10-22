package app.englishspeaker.ui.importer

import android.content.ClipboardManager
import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.englishspeaker.core.SourceKind
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
import app.englishspeaker.ui.components.pressable
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Type
import kotlin.math.roundToInt

/** 粗略统计：按句末标点切句、按空白计词（只用于“共 N 句 · 预计约 N 秒”的提示）。 */
private fun estimate(body: String): Pair<Int, Int> {
    val text = body.trim()
    if (text.isEmpty()) return 0 to 0
    val sentences = text.split(Regex("(?<=[.!?…])[\"”’)]*\\s+|\\n+")).count { it.isNotBlank() }
    val words = text.split(Regex("\\s+")).count { it.isNotBlank() }
    // 约 150 词/分钟 → 每词 0.4 秒；取整到 5 秒
    val secs = maxOf(5, ((words * 0.4) / 5).roundToInt() * 5)
    return sentences to secs
}

/** 导入（UI_SPEC §4.6）：分段（粘贴文本 / 文件 / 剪贴板）· 标题 · 正文 · 音色 + 导入后立即播放 · 开始转换。 */
@Composable
fun ImportSheet(vm: AppViewModel, onDismiss: () -> Unit) {
    val p = LocalPalette.current
    val repo = vm.repo
    val context = LocalContext.current
    var mode by remember { mutableIntStateOf(0) }
    var title by remember { mutableStateOf(vm.importPrefill?.first ?: "") }
    var body by remember { mutableStateOf(vm.importPrefill?.second ?: "") }
    androidx.compose.runtime.DisposableEffect(Unit) { onDispose { vm.importPrefill = null } }
    var source by remember { mutableStateOf(SourceKind.PASTE) }
    var sourceRef by remember { mutableStateOf<String?>(null) }
    val defaultVoice by repo.defaultVoice.collectAsState()
    var voice by remember { mutableStateOf<String?>(null) }
    val voices by repo.voices.collectAsState()
    var voiceMenu by remember { mutableStateOf(false) }
    var playNow by remember { mutableStateOf(vm.importPlayNow) }
    val chosenVoice = voice ?: defaultVoice

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) {
            val (name, text) = readText(context, uri)
            if (!text.isNullOrBlank()) {
                body = text
                if (title.isBlank()) title = name?.substringBeforeLast('.') ?: ""
                source = SourceKind.FILE
                sourceRef = name
            }
        }
    }

    AppSheet(onDismiss) { close ->
        Column(
            Modifier
                .fillMaxHeight(0.95f)
                .imePadding()
                .verticalScroll(rememberScrollState())
                .padding(start = 20.dp, end = 20.dp, top = 12.dp, bottom = 34.dp)
                .navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            SheetHeader("导入文章", close)
            Segmented(
                listOf("粘贴文本", "文件", "剪贴板"), mode,
                { i ->
                    mode = i
                    when (i) {
                        1 -> picker.launch(arrayOf("text/plain", "text/markdown", "text/*"))
                        2 -> {
                            val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                            val t = cm.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()
                            if (!t.isNullOrBlank()) { body = t; source = SourceKind.PASTE; sourceRef = null }
                        }
                        else -> { source = SourceKind.PASTE; sourceRef = null }
                    }
                },
                containerBg = p.bg, selectedBg = p.card, bordered = false, itemHeight = 38.dp,
            )
            Field("标题（可选）", title, { title = it }, "留空则使用第一句", singleLine = true)
            Field("正文", body, { body = it }, "粘贴或输入英文文章", singleLine = false)
            GroupCard {
                GroupRow("音色", modifier = Modifier.height(52.dp)) {
                    Box {
                        Chip("${repo.voiceLabel(chosenVoice)} ›", ChipKind.Neutral) { voiceMenu = true }
                        DropdownMenu(voiceMenu, { voiceMenu = false }, containerColor = p.surface, shape = RoundedCornerShape(16.dp)) {
                            voices.forEach { v ->
                                DropdownMenuItem(
                                    text = { Text(repo.voiceLabel(v.id), style = Type.body, color = if (v.id == chosenVoice) p.accentText else p.t1) },
                                    onClick = { voice = v.id; voiceMenu = false },
                                )
                            }
                        }
                    }
                }
                GroupRow("导入后立即播放", divider = false, modifier = Modifier.height(52.dp)) { Toggle(playNow, { playNow = it }, "导入后立即播放") }
            }
            val (n, secs) = estimate(body)
            Text(
                if (n == 0) "离线转换，不上传" else "共 $n 句 · 预计约 $secs 秒 · 离线转换，不上传",
                style = Type.secondary, color = p.t2, textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth(),
            )
            val enabled = body.isNotBlank()
            Box(
                Modifier
                    .fillMaxWidth()
                    .height(54.dp)
                    .alpha(if (enabled) 1f else 0.5f)
                    .clip(CircleShape)
                    .background(p.accent)
                    .pressable(enabled = enabled) {
                        repo.importAndMaybePlay(title, body, source, sourceRef, chosenVoice, playNow)?.let {
                            if (playNow) vm.playerOpen = true
                        }
                        close()
                    },
                contentAlignment = Alignment.Center,
            ) { Text("开始转换", style = Type.rowTitle.copy(fontSize = 16.sp), color = p.accentInk) }
        }
    }
}

@Composable
private fun Field(label: String, value: String, onChange: (String) -> Unit, placeholder: String, singleLine: Boolean) {
    val p = LocalPalette.current
    val shape = RoundedCornerShape(14.dp)
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Label(label)
        Box(
            Modifier
                .fillMaxWidth()
                .then(if (singleLine) Modifier.height(48.dp) else Modifier.heightIn(min = 180.dp))
                .background(p.card, shape)
                .border(1.dp, p.line, shape)
                .padding(14.dp),
            contentAlignment = if (singleLine) Alignment.CenterStart else Alignment.TopStart,
        ) {
            val style = if (singleLine) Type.body else Type.body.copy(lineHeight = 24.sp)
            if (value.isEmpty()) Text(placeholder, style = style, color = p.t3)
            BasicTextField(
                value, onChange,
                textStyle = style.copy(color = p.t1),
                cursorBrush = SolidColor(p.accent),
                singleLine = singleLine,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

private fun readText(context: Context, uri: Uri): Pair<String?, String?> {
    val name = runCatching {
        context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
            if (it.moveToFirst()) it.getString(0) else null
        }
    }.getOrNull()
    val text = runCatching {
        context.contentResolver.openInputStream(uri)?.use { it.readBytes().toString(Charsets.UTF_8) }
    }.getOrNull()
    return name to text
}
