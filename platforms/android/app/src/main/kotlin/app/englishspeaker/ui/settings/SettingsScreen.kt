package app.englishspeaker.ui.settings

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.TabChrome
import app.englishspeaker.ui.components.Chip
import app.englishspeaker.ui.components.ChipKind
import app.englishspeaker.ui.components.GroupCard
import app.englishspeaker.ui.components.GroupRow
import app.englishspeaker.ui.components.LargeTitle
import app.englishspeaker.ui.components.Segmented
import app.englishspeaker.ui.library.ReportScroll
import app.englishspeaker.ui.player.ReadingSettingsContent
import app.englishspeaker.ui.player.ThemeSegment
import app.englishspeaker.ui.translate.TranslationSettings
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Type
import java.util.Locale

private val GB = 1024uL * 1024uL * 1024uL
private val limits = listOf(1uL * GB, 2uL * GB, 5uL * GB, ULong.MAX_VALUE)

fun formatBytes(b: ULong): String {
    val v = b.toDouble()
    return when {
        v >= 1024.0 * 1024 * 1024 -> String.format(Locale.US, "%.1f GB", v / (1024.0 * 1024 * 1024))
        v >= 1024.0 * 1024 -> String.format(Locale.US, "%.0f MB", v / (1024.0 * 1024))
        v >= 1024.0 -> String.format(Locale.US, "%.0f KB", v / 1024.0)
        else -> "${b} B"
    }
}

/** 设置 Tab：外观 · 阅读 · 翻译 · 朗读 · 本地缓存 · 关于。 */
@Composable
fun SettingsScreen(vm: AppViewModel, chrome: TabChrome) {
    val p = LocalPalette.current
    val repo = vm.repo
    val st = vm.settings
    val theme by st.theme.collectAsState()
    val state = rememberLazyListState()
    ReportScroll(state, chrome)
    LaunchedEffect(Unit) { repo.refreshCache() }
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    LazyColumn(
        state = state,
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = top + 14.dp, bottom = chrome.bottomPadding),
        verticalArrangement = Arrangement.spacedBy(22.dp),
    ) {
        item { LargeTitle("设置") }
        item { Section("外观") { ThemeSegment(theme) { st.setTheme(it) } } }
        item { Section("阅读") { ReadingSettingsContent(vm, showTheme = false) } }
        item { Section("翻译") { TranslationSettings(repo) } }
        item { Section("朗读") { Speech(vm) } }
        item { Section("本地缓存") { Cache(vm) } }
        item { Section("关于") { About() } }
    }
}

@Composable
private fun Section(title: String, content: @Composable () -> Unit) {
    val p = LocalPalette.current
    Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
        Text(title, style = Type.section, color = p.t1)
        content()
    }
}

@Composable
private fun Speech(vm: AppViewModel) {
    val repo = vm.repo
    val voices by repo.voices.collectAsState()
    val voice by repo.defaultVoice.collectAsState()
    val rate by vm.settings.defaultRate.collectAsState()
    var menu by remember { mutableStateOf(false) }
    val p = LocalPalette.current
    GroupCard {
        GroupRow("默认音色") {
            Box {
                Chip("${repo.voiceLabel(voice)} ›", ChipKind.Neutral) { menu = true }
                DropdownMenu(menu, { menu = false }, containerColor = p.surface, shape = RoundedCornerShape(16.dp)) {
                    voices.forEach { v ->
                        DropdownMenuItem(
                            text = { Text(repo.voiceLabel(v.id), style = Type.body, color = if (v.id == voice) p.accentText else p.t1) },
                            onClick = { repo.setDefaultVoice(v.id); menu = false },
                        )
                    }
                }
            }
        }
        GroupRow("默认倍速", divider = false) {
            Chip("${rateLabel(rate)} ›", ChipKind.Neutral) {
                val next = nextRate(rate)
                vm.settings.setDefaultRate(next)
                repo.setRate(next)
            }
        }
    }
}

val rates = listOf(0.75f, 1.0f, 1.25f, 1.5f)
fun nextRate(r: Float): Float = rates[(rates.indexOfFirst { kotlin.math.abs(it - r) < 0.01f } + 1).mod(rates.size)]
fun rateLabel(r: Float): String = (if (r == 1f) "1.0" else (if (r * 100 % 10 == 0f) String.format(Locale.US, "%.1f", r) else String.format(Locale.US, "%.2f", r))) + "×"

@Composable
private fun Cache(vm: AppViewModel) {
    val p = LocalPalette.current
    val repo = vm.repo
    val stats by repo.cache.collectAsState()
    var confirm by remember { mutableStateOf(false) }
    var cleared by remember { mutableStateOf<Int?>(null) }
    val s = stats
    GroupCard {
        Column(Modifier.padding(vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            val used = s?.usedBytes ?: 0uL
            val limit = s?.limitBytes ?: GB
            val pinned = s?.pinnedBytes ?: 0uL
            val unlimited = limit == ULong.MAX_VALUE
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("已用 ${formatBytes(used)}", style = Type.body, color = p.t1, modifier = Modifier.weight(1f))
                Text(if (unlimited) "不限" else "上限 ${formatBytes(limit)}", style = Type.secondary, color = p.t2)
            }
            val denom = if (unlimited) maxOf(used, 1uL).toDouble() else limit.toDouble()
            val fu = (used.toDouble() / denom).toFloat().coerceIn(0f, 1f)
            val fp = (pinned.toDouble() / denom).toFloat().coerceIn(0f, 1f)
            Canvas(Modifier.fillMaxWidth().height(8.dp)) {
                val r = CornerRadius(size.height / 2)
                drawRoundRect(p.waveE, cornerRadius = r)
                if (fu > 0f) drawRoundRect(p.waveB, size = Size(size.width * fu, size.height), cornerRadius = r)
                if (fp > 0f) drawRoundRect(p.accent, size = Size(size.width * fp, size.height), cornerRadius = r)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalAlignment = Alignment.CenterVertically) {
                Legend(p.accent, "已保存 ${formatBytes(pinned)}")
                Legend(p.waveB, "可清理 ${formatBytes(if (used > pinned) used - pinned else 0uL)}")
                Text("${s?.entries ?: 0u} 项", style = Type.secondary, color = p.t2)
            }
        }
        androidx.compose.foundation.layout.Spacer(Modifier.fillMaxWidth().height(1.dp).background(p.line))
        Column(Modifier.padding(vertical = 14.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text("缓存上限", style = Type.body, color = p.t1)
            val sel = limits.indexOfFirst { it == s?.limitBytes }.let { if (it < 0) 0 else it }
            Segmented(listOf("1 GB", "2 GB", "5 GB", "不限"), sel, { repo.setCacheLimit(limits[it]) }, containerBg = p.bg, itemHeight = 38.dp)
        }
        androidx.compose.foundation.layout.Spacer(Modifier.fillMaxWidth().height(1.dp).background(p.line))
        GroupRow("清除未保存的缓存", divider = false, titleColor = p.dangerText, onClick = { confirm = true }) {
            cleared?.let { Text("已清除 $it 项", style = Type.secondary, color = p.t2) }
        }
    }
    Text("已保存的文章不会被自动清理。", style = Type.secondary, color = p.t2, modifier = Modifier.padding(start = 4.dp))
    if (confirm) {
        AlertDialog(
            onDismissRequest = { confirm = false },
            containerColor = p.surface,
            title = { Text("清除未保存的缓存？", style = Type.sheetTitle, color = p.t1) },
            text = { Text("未保存文章的音频将被删除，需要时会重新离线转换。已保存的文章不受影响。", style = Type.bodyMedium, color = p.t2) },
            confirmButton = {
                Chip("清除", ChipKind.Danger, height = 40.dp, hPad = 16.dp) { cleared = repo.clearCache(); confirm = false }
            },
            dismissButton = { Chip("取消", ChipKind.Neutral, height = 40.dp, hPad = 16.dp) { confirm = false } },
        )
    }
}

@Composable
private fun Legend(color: androidx.compose.ui.graphics.Color, text: String) {
    val p = LocalPalette.current
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Box(Modifier.size(8.dp).clip(CircleShape).background(color))
        Text(text, style = Type.secondary, color = p.t2)
    }
}

@Composable
private fun About() {
    val p = LocalPalette.current
    val context = LocalContext.current
    val version = remember {
        runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull() ?: "—"
    }
    var licenses by remember { mutableStateOf(false) }
    GroupCard {
        GroupRow("版本") { Text(version, style = Type.secondary, color = p.t2) }
        GroupRow("模型") { Text("Kokoro-82M v1.0 int8", style = Type.secondary, color = p.t2) }
        GroupRow("翻译模型") { Text("Opus-MT en-zh int8 · ECDICT", style = Type.secondary, color = p.t2) }
        GroupRow("隐私") { Text("完全离线，文章不会上传", style = Type.secondary, color = p.t2) }
        GroupRow("开源许可", divider = licenses, onClick = { licenses = !licenses }) {
            Text(if (licenses) "收起" else "查看 ›", style = Type.secondary, color = p.t2)
        }
        if (licenses) {
            Column(Modifier.padding(vertical = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                listOf(
                    "Kokoro-82M 模型 · Apache-2.0",
                    "ONNX Runtime · MIT",
                    "misaki 词典 · Apache-2.0",
                    "Opus-MT en-zh 翻译模型 · Apache-2.0",
                    "ECDICT 词典 · MIT",
                    "Qwen2.5-0.5B-Instruct（按需下载）· Apache-2.0",
                    "llama.cpp · MIT",
                    "Space Grotesk · Manrope · Noto Sans · SIL OFL 1.1",
                    "UniFFI · MPL-2.0",
                    "Haze · Apache-2.0",
                    "AndroidX / Jetpack Compose / Media3 · Apache-2.0",
                ).forEach { Text(it, style = Type.secondary, color = p.t2) }
            }
        }
    }
}

