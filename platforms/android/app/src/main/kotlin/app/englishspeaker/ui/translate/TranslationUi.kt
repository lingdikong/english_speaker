package app.englishspeaker.ui.translate

import android.content.Context
import android.net.ConnectivityManager
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.englishspeaker.core.DictEntry
import app.englishspeaker.core.LlmState
import app.englishspeaker.core.LlmStatus
import app.englishspeaker.core.TranslationEngine
import app.englishspeaker.core.TranslatorStatus
import app.englishspeaker.data.SpeakerRepository
import app.englishspeaker.ui.components.AppSheet
import app.englishspeaker.ui.components.Chip
import app.englishspeaker.ui.components.ChipKind
import app.englishspeaker.ui.components.GroupCard
import app.englishspeaker.ui.components.GroupRow
import app.englishspeaker.ui.components.IconBtn
import app.englishspeaker.ui.components.SheetHeader
import app.englishspeaker.ui.components.Toggle
import app.englishspeaker.ui.components.card
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.settings.formatBytes
import app.englishspeaker.ui.theme.Display
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Type

// 翻译与查词（核心层双模：Fast = Opus-MT + ECDICT，Smart = 本地 Qwen）。
// 平台层只负责展示：译文、词典释义、举一反三、句子解析、生词本、术语表、模型下载。

// —— 原文里的译文 / 句子解析 ——

/** 句子下方的中文译文（双语模式）。比原文小一号，跟随整句明暗。 */
@Composable
fun TranslationLine(text: String?, fontSize: Float, current: Boolean, modifier: Modifier = Modifier) {
    val p = LocalPalette.current
    val size = (fontSize * 0.66f).coerceAtLeast(13f)
    Text(
        text ?: if (current) "翻译中…" else " ",
        style = Type.bodyMedium.copy(fontSize = size.sp, lineHeight = (size * 1.6f).sp),
        color = if (current) p.t2 else p.other,
        modifier = modifier.padding(top = 6.dp),
    )
}

/** 当前句下方：“解析这句”按钮 / 解析结果（智能模式）。 */
@Composable
fun SentenceAnalysisPanel(repo: SpeakerRepository, sentence: Int) {
    val p = LocalPalette.current
    val analyses by repo.analyses.collectAsState()
    var open by remember(sentence) { mutableStateOf(false) }
    if (!open) {
        Chip("解析这句", ChipKind.Violet, height = 30.dp, icon = Icons.line("sparkle", 2f), iconSize = 14.dp) {
            open = true
            repo.requestAnalysis(sentence)
        }
        return
    }
    val r = analyses[sentence]
    Column(
        Modifier.fillMaxWidth().card(p, 18.dp).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        val a = r?.value
        when {
            a != null -> {
                Text(a.translation, style = Type.body, color = p.t1)
                if (a.structure.isNotBlank()) Text(
                    buildAnnotatedString {
                        withStyle(SpanStyle(color = p.violetText, fontWeight = FontWeight.Bold)) { append("结构  ") }
                        append(a.structure)
                    },
                    style = Type.secondary.copy(fontSize = 14.sp, lineHeight = 22.sp), color = p.t2,
                )
                a.points.forEach { pt ->
                    Text(
                        buildAnnotatedString {
                            withStyle(SpanStyle(color = p.violetText, fontWeight = FontWeight.Bold)) { append(pt.text + "  ") }
                            append(pt.note)
                        },
                        style = Type.secondary.copy(fontSize = 14.sp, lineHeight = 22.sp), color = p.t2,
                    )
                }
            }
            r?.error != null -> Text("解析失败：${r.error}", style = Type.secondary, color = p.t2)
            else -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                CircularProgressIndicator(Modifier.width(16.dp).height(16.dp), color = p.accent, strokeWidth = 2.dp)
                Text("正在用本地模型解析这句…", style = Type.secondary, color = p.t2)
            }
        }
    }
}

// —— 点词面板：词典 / 举一反三 ——

/** 离线词典释义：变形说明 · 前 3 条释义 · 考纲标签。 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun DictionarySection(entry: DictEntry) {
    val p = LocalPalette.current
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        entry.inflection?.let { infl ->
            Text(
                buildAnnotatedString {
                    withStyle(SpanStyle(color = p.t1, fontWeight = FontWeight.Bold)) { append(entry.query) }
                    append(" 是 ")
                    withStyle(SpanStyle(color = p.t1, fontWeight = FontWeight.Bold)) { append(entry.word) }
                    append(" 的$infl")
                },
                style = Type.secondary, color = p.t2,
            )
        }
        entry.senses.take(3).forEach { s ->
            Text(
                buildAnnotatedString {
                    if (s.pos.isNotEmpty()) withStyle(SpanStyle(color = p.t2, fontStyle = FontStyle.Italic, fontSize = 13.sp)) { append(s.pos + "  ") }
                    append(s.meaning)
                },
                style = Type.bodyMedium.copy(lineHeight = 22.sp), color = p.t1,
            )
        }
        if (entry.tags.isNotEmpty()) FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.padding(top = 2.dp)) {
            entry.tags.forEach { Chip(it, ChipKind.Ghost, height = 22.dp, hPad = 8.dp, style = Type.chip.copy(fontSize = 11.sp)) }
        }
    }
}

/** 举一反三：语境义 · 常用搭配 · 地道例句（智能模式，后台生成后经事件送达）。 */
@Composable
fun InsightSection(repo: SpeakerRepository, sentence: Int, word: Int) {
    val p = LocalPalette.current
    val insights by repo.insights.collectAsState()
    val r = insights["$sentence:$word"]
    val v = r?.value
    when {
        v != null -> Column(
            Modifier.fillMaxWidth().card(p, 16.dp).padding(horizontal = 16.dp, vertical = 14.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Chip("语境义", ChipKind.Violet, height = 24.dp, style = Type.chip.copy(fontSize = 11.sp))
                if (v.pos.isNotEmpty()) Text(v.pos, style = Type.secondary.copy(fontStyle = FontStyle.Italic), color = p.t2)
                Text(v.meaning, style = Type.rowTitle, color = p.t1)
            }
            if (v.collocations.isNotEmpty()) Text(v.collocations.joinToString("  ·  "), style = Type.rowTitle.copy(fontSize = 14.sp), color = p.violetText)
            v.examples.forEach { ex ->
                Column {
                    Text(ex.en, style = Type.secondary, color = p.t1)
                    Text(ex.zh, style = Type.secondary, color = p.t2)
                }
            }
        }
        r?.loading == true -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            CircularProgressIndicator(Modifier.width(16.dp).height(16.dp), color = p.accent, strokeWidth = 2.dp)
            Text("正在结合这句话分析…", style = Type.secondary, color = p.t2)
        }
        else -> Chip(
            if (r?.error == null) "举一反三" else "举一反三（重试）", ChipKind.Violet, height = 32.dp,
            icon = Icons.line("sparkle", 2f), iconSize = 14.dp,
        ) { repo.requestInsight(sentence, word) }
    }
}

// —— 设置页“翻译”组 ——

private fun onMetered(context: Context): Boolean =
    (context.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager)?.isActiveNetworkMetered == true

@Composable
fun TranslationSettings(repo: SpeakerRepository) {
    val p = LocalPalette.current
    val context = LocalContext.current
    val t by repo.translator.collectAsState()
    val bilingual by repo.bilingual.collectAsState()
    val glossary by repo.glossary.collectAsState()
    val vocab by repo.vocab.collectAsState()
    var confirmDownload by remember { mutableStateOf(false) }
    var confirmDelete by remember { mutableStateOf(false) }
    var showVocab by remember { mutableStateOf(false) }
    var showGlossary by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { repo.refreshGlossary() }
    val status = t ?: return
    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        GroupCard {
            GroupRow("双语显示") { Toggle(bilingual, { repo.setBilingual(it) }, "双语显示") }
            GroupRow("智能翻译") {
                Toggle(status.smartEnabled, { on ->
                    if (on && status.llm.state != LlmState.DOWNLOADED && status.llm.state != LlmState.DOWNLOADING) confirmDownload = true
                    else repo.setSmart(on)
                }, "智能翻译")
            }
            ModelRow(status.llm, onDownload = { confirmDownload = true }, onCancel = { repo.cancelLlmDownload() }, onDelete = { confirmDelete = true })
            GroupRow("术语表", onClick = { repo.refreshGlossary(); showGlossary = true }) {
                Chip("${glossary.size} 条 ›", ChipKind.Neutral) { repo.refreshGlossary(); showGlossary = true }
            }
            GroupRow("生词本", divider = false, onClick = { showVocab = true }) {
                Chip("${vocab.size} 个词 ›", ChipKind.Neutral) { showVocab = true }
            }
        }
        Text(note(status), style = Type.secondary, color = p.t2)
    }
    if (confirmDownload) {
        val metered = remember { onMetered(context) }
        ConfirmDialog(
            title = "下载智能翻译模型？",
            body = "需要下载 ${status.llm.modelName}（约 ${formatBytes(status.llm.totalBytes)}）" +
                (if (metered) "，当前正在使用移动数据，建议连接 Wi-Fi 后下载" else "") +
                "。下载后完全离线运行，约占用 500 MB 内存，可随时删除。",
            ok = "下载并开启",
            onOk = { repo.setSmart(true); repo.downloadLlm() },
            onDismiss = { confirmDownload = false },
        )
    }
    if (confirmDelete) {
        ConfirmDialog(
            title = "删除智能翻译模型？",
            body = "删除后整句翻译退回极速模式，已生成的译文与解析会保留。",
            ok = "删除", danger = true,
            onOk = { repo.deleteLlm() },
            onDismiss = { confirmDelete = false },
        )
    }
    if (showVocab) VocabSheet(repo) { showVocab = false }
    if (showGlossary) GlossarySheet(repo) { showGlossary = false }
}

private fun note(t: TranslatorStatus): String {
    var s = when (t.active) {
        TranslationEngine.SMART -> "整句翻译：智能翻译（以极速译文为参考精修）。"
        TranslationEngine.FAST -> "整句翻译：极速翻译（Opus-MT，内置）。"
        null -> ""
    }
    if (t.smartEnabled && t.active != TranslationEngine.SMART) s += "智能翻译模型下载完成后自动启用。"
    return s
}

@Composable
private fun ModelRow(llm: LlmStatus, onDownload: () -> Unit, onCancel: () -> Unit, onDelete: () -> Unit) {
    val p = LocalPalette.current
    val frac = if (llm.totalBytes > 0u) llm.downloadedBytes.toFloat() / llm.totalBytes.toFloat() else 0f
    Row(Modifier.fillMaxWidth().padding(vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(llm.modelName, style = Type.body.copy(fontSize = 14.sp), color = p.t1)
                if (llm.loaded) Chip("运行中", ChipKind.Accent, height = 20.dp, hPad = 7.dp, style = Type.chip.copy(fontSize = 10.sp))
            }
            when (llm.state) {
                LlmState.DOWNLOADING -> {
                    LinearProgressIndicator(progress = { frac }, modifier = Modifier.fillMaxWidth().height(6.dp),
                        color = p.accent, trackColor = p.waveE, drawStopIndicator = {})
                    Text("正在下载 ${formatBytes(llm.downloadedBytes)} / ${formatBytes(llm.totalBytes)}", style = Type.secondary.copy(fontSize = 12.sp), color = p.t2)
                }
                LlmState.DOWNLOADED -> Text("已下载 · ${formatBytes(llm.totalBytes)} · 完全离线运行", style = Type.secondary.copy(fontSize = 12.sp), color = p.t2)
                LlmState.FAILED -> Text("下载失败：${llm.error.orEmpty()}", style = Type.secondary.copy(fontSize = 12.sp), color = p.dangerText)
                LlmState.NOT_DOWNLOADED -> Text(
                    "未下载 · 约 ${formatBytes(llm.totalBytes)}" +
                        (if (llm.downloadedBytes > 0u) "（已下载 ${formatBytes(llm.downloadedBytes)}，可继续）" else ""),
                    style = Type.secondary.copy(fontSize = 12.sp), color = p.t2,
                )
            }
        }
        Spacer(Modifier.width(12.dp))
        when (llm.state) {
            LlmState.DOWNLOADING -> Chip("暂停", ChipKind.Neutral, onClick = onCancel)
            LlmState.DOWNLOADED -> Chip("删除", ChipKind.Danger, onClick = onDelete)
            else -> Chip(if (llm.downloadedBytes > 0u || llm.state == LlmState.FAILED) "继续下载" else "下载", ChipKind.Accent, onClick = onDownload)
        }
    }
}

@Composable
private fun ConfirmDialog(title: String, body: String, ok: String, danger: Boolean = false, onOk: () -> Unit, onDismiss: () -> Unit) {
    val p = LocalPalette.current
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = p.surface,
        title = { Text(title, style = Type.sheetTitle, color = p.t1) },
        text = { Text(body, style = Type.bodyMedium, color = p.t2) },
        confirmButton = {
            TextButton({ onOk(); onDismiss() }) { Text(ok, style = Type.rowTitle, color = if (danger) p.dangerText else p.accentText) }
        },
        dismissButton = { TextButton(onDismiss) { Text("取消", style = Type.rowTitle, color = p.t2) } },
    )
}

// —— 生词本 ——

@Composable
fun VocabSheet(repo: SpeakerRepository, onDismiss: () -> Unit) {
    val p = LocalPalette.current
    val vocab by repo.vocab.collectAsState()
    AppSheet(onDismiss) { close ->
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(start = 20.dp, end = 20.dp, top = 8.dp, bottom = 28.dp).navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            SheetHeader("生词本", close)
            if (vocab.isEmpty()) Text(
                "还没有生词。播放时点原文里的词，在弹出的卡片上点 ☆ 收藏。",
                style = Type.secondary.copy(fontSize = 14.sp), color = p.t2, modifier = Modifier.padding(top = 40.dp),
            )
            vocab.forEach { v ->
                Column(Modifier.fillMaxWidth().card(p, 20.dp).padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(v.word, style = Type.section.copy(fontFamily = Display, fontSize = 22.sp), color = p.t1)
                        if (v.phonetic.isNotEmpty()) Text("  /${v.phonetic}/", style = Type.ipa.copy(fontSize = 15.sp), color = p.violetText)
                        Spacer(Modifier.weight(1f))
                        IconBtn(Icons.line("trash"), "移出生词本", size = 32.dp, iconSize = 17.dp, tint = p.t2) { repo.removeVocab(v.word) }
                    }
                    if (v.meaning.isNotEmpty()) Text(v.meaning, style = Type.secondary.copy(fontSize = 14.sp), color = p.t1)
                    Text(v.sentence, style = Type.secondary.copy(fontSize = 14.sp, lineHeight = 21.sp), color = p.t2)
                    val id = v.articleId
                    val s = v.sentenceIndex
                    if (id != null && s != null) Chip(
                        "${v.articleTitle ?: "原文"} · 第 ${s.toInt() + 1} 句", ChipKind.Accent, height = 28.dp,
                        icon = Icons.line("vol", 2f), iconSize = 14.dp,
                    ) {
                        repo.play(id)
                        repo.seekSentence(s.toInt())
                        close()
                    }
                }
            }
        }
    }
}

// —— 术语表 ——

@Composable
fun GlossarySheet(repo: SpeakerRepository, onDismiss: () -> Unit) {
    val p = LocalPalette.current
    val glossary by repo.glossary.collectAsState()
    var term by remember { mutableStateOf("") }
    var zh by remember { mutableStateOf("") }
    AppSheet(onDismiss) { close ->
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(start = 20.dp, end = 20.dp, top = 8.dp, bottom = 28.dp).navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            SheetHeader("术语表", close)
            GroupCard {
                glossary.forEach { g ->
                    GroupRow("${g.term}  →  ${g.translation}") {
                        IconBtn(Icons.line("close", 2f), "删除术语 ${g.term}", size = 32.dp, iconSize = 15.dp, tint = p.t2) { repo.removeGlossary(g.term) }
                    }
                }
                Row(Modifier.fillMaxWidth().height(54.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    Field(term, { term = it }, "英文术语，如 token", Modifier.weight(1f))
                    Field(zh, { zh = it }, "固定译法，如 令牌", Modifier.weight(1f))
                    Chip("添加", ChipKind.Accent) {
                        if (term.isNotBlank() && zh.isNotBlank() && repo.addGlossary(term, zh)) { term = ""; zh = "" }
                    }
                }
            }
            Text("整句翻译遇到术语时一律使用固定译法；修改后含该术语的句子会自动重新翻译。", style = Type.secondary.copy(fontSize = 12.sp), color = p.t2)
        }
    }
}

@Composable
private fun Field(value: String, onChange: (String) -> Unit, hint: String, modifier: Modifier) {
    val p = LocalPalette.current
    BasicTextField(
        value, onChange, singleLine = true,
        textStyle = Type.body.copy(color = p.t1, fontSize = 14.sp),
        cursorBrush = SolidColor(p.accent),
        modifier = modifier.height(36.dp).background(p.control, RoundedCornerShape(10.dp)).padding(horizontal = 12.dp, vertical = 8.dp),
        decorationBox = { inner ->
            if (value.isEmpty()) Text(hint, style = Type.secondary.copy(fontSize = 14.sp), color = p.t3, maxLines = 1)
            inner()
        },
    )
}

/** 点词面板的收藏按钮（☆）：按词典原形判断是否已收藏。 */
@Composable
fun VocabStar(repo: SpeakerRepository, key: String, sentence: Int, word: Int) {
    val p = LocalPalette.current
    val vocab by repo.vocab.collectAsState()
    val saved = vocab.any { it.word == key.lowercase() }
    IconBtn(
        Icons.line("star", 1.9f), if (saved) "从生词本移除" else "加入生词本", fill = true, iconSize = 19.dp,
        tint = if (saved) p.accentText else p.t1,
    ) { if (saved) repo.removeVocab(key) else repo.addVocab(sentence, word) }
}
