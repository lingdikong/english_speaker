package app.englishspeaker.ui.player

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.englishspeaker.core.Accent
import app.englishspeaker.core.Link
import app.englishspeaker.core.WordSpan
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.WordTarget
import app.englishspeaker.ui.components.AppSheet
import app.englishspeaker.ui.components.Chip
import app.englishspeaker.ui.components.ChipKind
import app.englishspeaker.ui.components.IconBtn
import app.englishspeaker.ui.components.card
import app.englishspeaker.ui.components.pressable
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.theme.Display
import app.englishspeaker.ui.theme.Ipa
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Palette
import app.englishspeaker.ui.theme.Type
import app.englishspeaker.ui.theme.Ui
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

private fun clean(t: String) = t.trim { !it.isLetterOrDigit() && it != '\'' && it != '’' }

/** 读音现象分类（用于类型 chip、说明与“同类”示例）。 */
private enum class Kind(val label: String, val examples: List<String>) {
    Flap("跨词闪音", listOf("get‿it", "what‿are", "a lot‿of")),
    ConsonantVowel("辅音连元音", listOf("pick‿it‿up", "turn‿off", "look‿at")),
    GlideJ("加音 /j/ 连读", listOf("see‿it", "my‿own", "the‿end")),
    GlideW("加音 /w/ 连读", listOf("go‿on", "do‿it", "how‿about")),
    LinkingR("连接 r", listOf("far‿away", "more‿often", "after‿all")),
    SameSound("同音合并", listOf("big‿game", "bus‿stop", "hot‿tea")),
    Assimilation("同化", listOf("did‿you", "would‿you", "this‿year")),
    Weak("弱读", listOf("fish and chips", "a cup of tea", "want to go")),
    WordFlap("闪音", listOf("water", "better", "city")),
    Elided("省音", listOf("next day", "last night", "must be")),
    Plain("词典读音", emptyList()),
}

private fun linkKind(l: Link) = when (l) {
    Link.CONSONANT_VOWEL -> Kind.ConsonantVowel
    Link.GLIDE_J -> Kind.GlideJ
    Link.GLIDE_W -> Kind.GlideW
    Link.LINKING_R -> Kind.LinkingR
    Link.SAME_SOUND -> Kind.SameSound
    Link.FLAP -> Kind.Flap
    Link.ASSIMILATION -> Kind.Assimilation
}

/** 中文说明：词用 t1 粗体，IPA 用 violet-text。 */
private fun explain(k: Kind, a: String, b: String?, ipa: String, weak: String?, p: Palette): AnnotatedString = buildAnnotatedString {
    fun w(s: String) = withStyle(SpanStyle(color = p.t1, fontWeight = FontWeight.Bold)) { append(s) }
    fun i(s: String) = withStyle(SpanStyle(color = p.violetText, fontFamily = Ipa)) { append(s) }
    val bb = b ?: ""
    when (k) {
        Kind.Flap -> { w(a); append(" 末尾的 t 夹在两个元音之间，读得像轻快的 d，并和后面的 "); w(bb); append(" 连成一个音节") }
        Kind.ConsonantVowel -> { w(a); append(" 末尾的辅音直接带到 "); w(bb); append(" 开头的元音上，两个词连成一个音节") }
        Kind.GlideJ -> { w(a); append(" 以 /i/ 类元音结尾，后面的 "); w(bb); append(" 以元音开头，中间自然带出一个轻轻的 /j/") }
        Kind.GlideW -> { w(a); append(" 以 /u/ 类元音结尾，后面的 "); w(bb); append(" 以元音开头，中间自然带出一个轻轻的 /w/") }
        Kind.LinkingR -> { append("英式发音里 "); w(a); append(" 末尾的 r 平时不读，但后面接元音开头的 "); w(bb); append(" 时要读出来") }
        Kind.SameSound -> { w(a); append(" 的结尾和 "); w(bb); append(" 的开头是同一个辅音，只读一次，稍微拉长") }
        Kind.Assimilation -> { w(a); append(" 末尾的音受后面 "); w(bb); append(" 的影响而改变，两个音融合在一起") }
        Kind.Weak -> { w(a); append(" 在句中是虚词，不重读，读成弱读形式"); if (weak != null) { append(" "); i("/$weak/") } }
        Kind.WordFlap -> { w(a); append(" 中的 t/d 夹在元音之间，读成轻快的闪音 "); i("/ɾ/") }
        Kind.Elided -> { w(a); append(" 末尾的 t/d 在后面的辅音前几乎不发音") }
        Kind.Plain -> { w(a); append(" 在这里按词典读音朗读，没有明显的连读或弱读") }
    }
    if (ipa.isNotEmpty() && k != Kind.Weak && k != Kind.WordFlap) { append("："); i("/$ipa/") }
    append("。")
}

/** “put‿it” 式文本：‿ 用 violet。 */
private fun linked(text: String, p: Palette, base: SpanStyle) = buildAnnotatedString {
    text.split('‿').forEachIndexed { i, part ->
        if (i > 0) withStyle(SpanStyle(color = p.violet, fontFamily = Ipa, fontWeight = FontWeight.Bold)) { append("‿") }
        withStyle(base) { append(part) }
    }
}

/** 点词读音（UI_SPEC §4.5）：词组 34 + IPA 20 + 发音按钮；类型 chip；中文说明；“同类”；从这里播放 / 0.75× 慢放这句。 */
@Composable
fun WordSheet(vm: AppViewModel, target: WordTarget, onDismiss: () -> Unit) {
    val p = LocalPalette.current
    val repo = vm.repo
    val timeline by repo.timeline.collectAsState()
    val scope = rememberCoroutineScope()
    val s = timeline?.sentences?.getOrNull(target.sentence) ?: run { onDismiss(); return }
    val words = s.words
    val w: WordSpan = words.getOrNull(target.word) ?: run { onDismiss(); return }
    // 连读词组：本词连向下一个词，或上一个词连到本词
    val prev = words.getOrNull(target.word - 1)
    val pair: Pair<WordSpan, WordSpan?> = when {
        w.link != null && target.word + 1 < words.size -> w to words[target.word + 1]
        prev?.link != null -> prev to w
        else -> w to null
    }
    val (first, second) = pair
    val kind = when {
        second != null -> linkKind(first.link!!)
        w.weakForm != null -> Kind.Weak
        w.flap -> Kind.WordFlap
        w.elided -> Kind.Elided
        else -> Kind.Plain
    }
    val a = clean(first.text)
    val b = second?.let { clean(it.text) }
    val phrase = if (b != null) "$a‿$b" else a
    val ipa = listOfNotNull(first.ipa.takeIf { it.isNotBlank() }, second?.ipa?.takeIf { it.isNotBlank() }).joinToString(" ")
    val article = repo.article(repo.player.value.articleId)
    val accent = repo.voice(article?.voiceId)?.accent
    val hasTimes = s.ready && first.endMs > first.startMs

    fun playRange(from: WordSpan, to: WordSpan) {
        scope.launch {
            val end = to.endMs.toLong()
            repo.seek(from.startMs.toLong())
            repo.play()
            withTimeoutOrNull(6000) { repo.position.first { it.positionMs >= end } }
            repo.pause()
        }
    }

    AppSheet(onDismiss) { close ->
        Column(
            Modifier.padding(start = 22.dp, end = 22.dp, top = 18.dp, bottom = 28.dp).navigationBarsPadding(),
            verticalArrangement = Arrangement.spacedBy(18.dp),
        ) {
            Row(verticalAlignment = Alignment.Top) {
                Column(Modifier.weight(1f)) {
                    Text(
                        linked(phrase, p, SpanStyle(color = p.t1)),
                        style = Type.largeTitle.copy(fontFamily = Display, fontSize = 34.sp, lineHeight = 40.sp, letterSpacing = 0.sp),
                    )
                    if (ipa.isNotEmpty()) Text("/$ipa/", style = Type.ipa, color = p.violetText, modifier = Modifier.padding(top = 4.dp))
                }
                IconBtn(Icons.line("vol", 1.9f), "播放这个词", fill = true, iconSize = 20.dp, enabled = hasTimes) {
                    playRange(first, second ?: first)
                }
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Chip(kind.label, ChipKind.Violet)
                Chip(if (accent == Accent.BRITISH) "英式发音" else "美式发音", ChipKind.Muted)
            }
            Text(explain(kind, a, b, ipa, w.weakForm, p), style = Type.bodyMedium, color = p.t2)
            if (kind.examples.isNotEmpty()) {
                val ex = buildAnnotatedString {
                    append("同类：")
                    kind.examples.forEachIndexed { i, e ->
                        if (i > 0) append(" · ")
                        append(linked(e, p, SpanStyle(color = p.t1, fontWeight = FontWeight.Bold, fontFamily = Ui)))
                    }
                }
                Text(
                    ex, style = Type.secondary.copy(fontSize = 14.sp, lineHeight = 22.sp), color = p.t2,
                    modifier = Modifier.fillMaxWidth().card(p, 16.dp).padding(horizontal = 16.dp, vertical = 14.dp),
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Box(
                    Modifier.weight(1f).height(48.dp).clip(CircleShape).background(p.accent)
                        .pressable(enabled = hasTimes) {
                            repo.seek(first.startMs.toLong()); repo.play(); close()
                        },
                    contentAlignment = Alignment.Center,
                ) { Text("从这里播放", style = Type.rowTitle, color = p.accentInk) }
                Box(
                    Modifier.weight(1f).height(48.dp).clip(CircleShape).background(p.control)
                        .pressable(enabled = s.ready) {
                            repo.setRate(0.75f); repo.seekSentence(target.sentence); close()
                        },
                    contentAlignment = Alignment.Center,
                ) { Text("0.75× 慢放这句", style = Type.rowTitle, color = p.t1) }
            }
        }
    }
}
