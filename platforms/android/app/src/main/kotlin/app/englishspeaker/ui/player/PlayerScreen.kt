package app.englishspeaker.ui.player

import android.os.SystemClock
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.animateScrollBy
import androidx.compose.foundation.interaction.collectIsDraggedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.englishspeaker.core.ArticleSummary
import app.englishspeaker.core.PlayerStatus
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.WordTarget
import app.englishspeaker.ui.components.Chip
import app.englishspeaker.ui.components.ChipKind
import app.englishspeaker.ui.components.FadeStops
import app.englishspeaker.ui.components.IconBtn
import app.englishspeaker.ui.components.PlayButton
import app.englishspeaker.ui.components.WaveCover
import app.englishspeaker.ui.components.WaveProgress
import app.englishspeaker.ui.components.currentCard
import app.englishspeaker.ui.components.fadeUnder
import app.englishspeaker.ui.components.fadeTop
import app.englishspeaker.ui.components.glass
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.settings.nextRate
import app.englishspeaker.ui.settings.rateLabel
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.LocalReduceMotion
import app.englishspeaker.ui.theme.Type
import app.englishspeaker.ui.theme.fadeSpec
import dev.chrisbanes.haze.hazeSource
import dev.chrisbanes.haze.rememberHazeState
import kotlinx.coroutines.delay

fun formatTime(ms: Long): String {
    val s = (ms / 1000).coerceAtLeast(0)
    return "${s / 60}:${(s % 60).toString().padStart(2, '0')}"
}

/** 播放页（UI_SPEC §4.5）：原文为中心，无大封面；当前句固定在约 35% 高度；玻璃控制面板；专注模式。 */
@Composable
fun PlayerScreen(vm: AppViewModel, a: ArticleSummary) {
    val p = LocalPalette.current
    val reduce = LocalReduceMotion.current
    val repo = vm.repo
    val player by repo.player.collectAsState()
    val pos by repo.position.collectAsState()
    val timeline by repo.timeline.collectAsState()
    val annotations by repo.showAnnotations.collectAsState()
    val focusEnabled by vm.settings.focusMode.collectAsState()
    val fontSize by vm.settings.fontSize.collectAsState()
    val lineHeight by vm.settings.lineHeight.collectAsState()
    val otherAlpha by vm.settings.otherAlpha.collectAsState()
    val loop by repo.loopSentence.collectAsState()
    val haze = rememberHazeState()
    val sentences = timeline?.sentences.orEmpty()
    val playing = player.status == PlayerStatus.PLAYING

    // —— 专注模式：播放中 5 秒无操作 → 收起次要控件；轻点任意处恢复 ——
    var lastTouch by remember { mutableLongStateOf(SystemClock.elapsedRealtime()) }
    var focused by remember { mutableStateOf(vm.forceFocus) }
    LaunchedEffect(vm.forceFocus) { if (vm.forceFocus) focused = true }
    LaunchedEffect(lastTouch, playing, focusEnabled, vm.forceFocus) {
        if (vm.forceFocus || !focusEnabled || !playing) return@LaunchedEffect
        delay(5000)
        focused = true
    }
    BackHandler(focused) { focused = false; vm.forceFocus = false; lastTouch = SystemClock.elapsedRealtime() }
    val chromeAlpha by animateFloatAsState(if (focused) 0f else 1f, fadeSpec(reduce, 250), label = "chrome")
    // 专注模式只收起外壳（头部/控制面板），跟读标注（‿ 连读、弱读、点线）要保留 ——
    // 与 iOS 一致：早先写成 annotations && !focused，专注模式下连读符号全没了。
    val showAnn = annotations

    BoxWithConstraints(
        Modifier
            .fillMaxSize()
            .background(p.bg)
            .pointerInput(Unit) {
                awaitPointerEventScope {
                    while (true) {
                        val e = awaitPointerEvent(PointerEventPass.Initial)
                        if (e.type == PointerEventType.Press) {
                            lastTouch = SystemClock.elapsedRealtime()
                            if (focused) {
                                focused = false
                                vm.forceFocus = false
                                e.changes.forEach { it.consume() }
                            }
                        }
                    }
                }
            }
    ) {
        val screenH = maxHeight
        val statusTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
        val navBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
        val headerH = statusTop + 8.dp + 44.dp + 8.dp + 48.dp
        val panelBottom = if (navBottom <= 30.dp) 10.dp else navBottom + 6.dp

        // —— 原文 ——
        Box(
            Modifier
                .fillMaxSize()
                .padding(top = headerH)
                .clipToBounds()
                .hazeSource(haze)
                .fadeUnder(FadeStops.mp, extraBottom = panelBottom - 10.dp)
                // 顶部在标题行下渐隐（约 56dp 渐变到透明）
                .fadeTop(6.dp, 50.dp)
        ) {
            // 当前句卡片顶部固定在屏幕约 32% 处（卡片中线约 35%）
            val anchor = (screenH * 0.32f - headerH).coerceAtLeast(24.dp)
            // 核心层在词与词的间隙里给 word = null：保持最后一个已开始的词高亮，避免胶囊闪烁
            val curIdx = pos.sentence ?: 0
            val effWord = pos.word ?: sentences.getOrNull(curIdx)?.words
                ?.indexOfLast { it.startMs.toLong() <= pos.positionMs }?.takeIf { it >= 0 && pos.sentence != null }
            Transcript(
                vm, sentences, curIdx, effWord, showAnn, fontSize.toFloat(), lineHeight,
                p.otherAt(otherAlpha), anchor, screenH,
                onWord = { s, w -> vm.word = WordTarget(s, w) },
            )
            // 专注模式时顶部再叠一层底色渐变（设计稿 focus 顶部 60）
            if (chromeAlpha < 1f) {
                Box(
                    Modifier.fillMaxWidth().height(60.dp)
                        .graphicsLayer { alpha = 1f - chromeAlpha }
                        .background(Brush.verticalGradient(listOf(p.bg, Color.Transparent)))
                )
            }
        }

        // —— 顶部两行 ——
        Column(Modifier.fillMaxWidth().padding(top = statusTop + 8.dp).graphicsLayer { alpha = chromeAlpha }) {
            var more by remember { mutableStateOf(false) }
            Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                IconBtn(Icons.line("down", 1.9f), "收起", iconSize = 24.dp, enabled = !focused) { vm.requestCollapse() }
                val total = sentences.size
                val cur = (pos.sentence ?: 0) + 1
                Text(
                    if (total > 0) "第 $cur / $total 句" else "准备中…",
                    style = Type.chip, color = p.t2,
                    modifier = Modifier.weight(1f), textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                )
                Box {
                    IconBtn(Icons.more, "更多", enabled = !focused) { more = true }
                    DropdownMenu(more, { more = false }, containerColor = p.surface, shape = RoundedCornerShape(16.dp)) {
                        DropdownMenuItem({ Text(if (a.pinned) "取消保存" else "保存（离线保留）", style = Type.body, color = p.t1) }, { repo.setPinned(a.id, !a.pinned); more = false })
                        DropdownMenuItem({ Text("待播清单", style = Type.body, color = p.t1) }, { vm.showUpNext = true; more = false })
                        DropdownMenuItem({ Text("阅读设置", style = Type.body, color = p.t1) }, { vm.showReading = true; more = false })
                    }
                }
            }
            Row(Modifier.fillMaxWidth().padding(start = 20.dp, end = 20.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                WaveCover(a.title, 48, p.accent)
                Spacer(Modifier.width(12.dp))
                Column(Modifier.weight(1f)) {
                    Text(a.title, style = Type.displayTitle.copy(fontSize = 17.sp), color = p.t1, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    Text(repo.voiceLabel(a.voiceId), style = Type.secondary, color = p.t2)
                }
                IconBtn(
                    if (a.pinned) Icons.saved else Icons.line("bookmark", 1.9f),
                    if (a.pinned) "已保存（点按取消）" else "保存",
                    fill = true, iconSize = 18.dp, tint = if (a.pinned) p.accent else p.t1, enabled = !focused,
                ) { repo.setPinned(a.id, !a.pinned) }
            }
        }

        // —— 玻璃控制面板 ——
        Column(
            Modifier
                .align(Alignment.BottomCenter)
                .padding(start = 10.dp, end = 10.dp, bottom = panelBottom)
                .fillMaxWidth()
                .glass(RoundedCornerShape(38.dp), haze)
                .padding(start = 18.dp, end = 18.dp, top = 16.dp, bottom = 20.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            val dur = player.durationMs.toLong().coerceAtLeast(1)
            val at = pos.positionMs.coerceAtMost(dur)
            Column {
                WaveProgress(
                    a.id, at.toFloat() / dur, pos.bufferedMs.toFloat() / dur,
                    onSeek = { f -> repo.seek((f * dur).toLong()) },
                )
                Row(Modifier.fillMaxWidth().padding(top = 6.dp)) {
                    Text(formatTime(at), style = Type.time, color = p.t2, modifier = Modifier.weight(1f))
                    Text((if (player.durationFinal) "" else "约 ") + "-" + formatTime(dur - at), style = Type.time, color = p.t2)
                }
            }
            AnimatedContent(focused, transitionSpec = { fadeIn(fadeSpec(reduce)) togetherWith fadeOut(fadeSpec(reduce)) }, label = "panel") { f ->
                if (f) {
                    Text(
                        "专注模式 · 轻点任意处显示控件", style = Type.secondary.copy(fontSize = 12.sp), color = p.t2,
                        textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                        modifier = Modifier.fillMaxWidth().padding(top = 2.dp, bottom = 6.dp),
                    )
                } else {
                    Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
                            Chip(rateLabel(player.rate), ChipKind.Neutral, height = 40.dp, hPad = 14.dp, style = Type.chip.copy(fontSize = 14.sp)) {
                                repo.setRate(nextRate(player.rate))
                            }
                            IconBtn(Icons.line("prev", 1.9f), "上一句", iconSize = 28.dp) { repo.previousSentence() }
                            PlayButton(playing, 72.dp, 28.dp, buffering = player.status == PlayerStatus.BUFFERING) { repo.toggle() }
                            IconBtn(Icons.line("next", 1.9f), "下一句", iconSize = 28.dp) { repo.nextSentence() }
                            IconBtn(
                                Icons.line("loop"), if (loop != null) "单句循环（已开启）" else "单句循环", iconSize = 24.dp,
                                tint = if (loop != null) p.accentText else p.t1,
                            ) { repo.toggleLoop() }
                        }
                        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                            Chip("跟读标注", if (annotations) ChipKind.Violet else ChipKind.Muted, height = 36.dp, icon = Icons.line("annot", 2f)) {
                                repo.setAnnotations(!annotations)
                            }
                            Spacer(Modifier.width(8.dp))
                            Chip("阅读设置", ChipKind.Muted, height = 36.dp, icon = Icons.line("textsize", 1.9f)) { vm.showReading = true }
                            Spacer(Modifier.weight(1f))
                            IconBtn(Icons.line("queue"), "待播清单") { vm.showUpNext = true }
                        }
                    }
                }
            }
        }
    }
}

/** 原文列表：当前句固定在 [anchor]；手动滚动后暂停跟随，3 秒后或点“回到当前句”恢复。 */
@Composable
private fun Transcript(
    vm: AppViewModel,
    sentences: List<app.englishspeaker.core.SentenceSpan>,
    current: Int?,
    word: Int?,
    annotations: Boolean,
    fontSize: Float,
    lineHeight: Float,
    otherColor: Color,
    anchor: androidx.compose.ui.unit.Dp,
    screenH: androidx.compose.ui.unit.Dp,
    onWord: (Int, Int) -> Unit,
) {
    val p = LocalPalette.current
    val reduce = LocalReduceMotion.current
    val repo = vm.repo
    val listState = rememberLazyListState()
    val dragged by listState.interactionSource.collectIsDraggedAsState()
    var following by remember { mutableStateOf(true) }
    var resumeTick by remember { mutableLongStateOf(0L) }
    LaunchedEffect(dragged, resumeTick) {
        if (dragged) following = false
        else if (!following) { delay(3000); following = true }
    }
    var jumped by remember { mutableStateOf(false) }
    LaunchedEffect(current, following, sentences.size) {
        val idx = current ?: 0
        if (!following || sentences.isEmpty()) return@LaunchedEffect
        val info = listState.layoutInfo.visibleItemsInfo.firstOrNull { it.index == idx }
        if (info != null && jumped && !reduce) {
            listState.animateScrollBy(info.offset.toFloat(), tween(280))
        } else {
            listState.scrollToItem(idx)
            jumped = true
        }
    }
    Box(Modifier.fillMaxSize()) {
        LazyColumn(
            state = listState,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 22.dp, end = 22.dp, top = anchor, bottom = screenH),
            verticalArrangement = Arrangement.spacedBy(22.dp),
        ) {
            itemsIndexed(sentences, key = { i, _ -> i }) { i, s ->
                val isCur = i == (current ?: -1)
                val newPara = i > 0 && s.paragraph != sentences[i - 1].paragraph
                Column(Modifier.padding(top = if (newPara) 10.dp else 0.dp)) {
                    if (isCur) {
                        Column(Modifier.fillMaxWidth().currentCard(p).padding(18.dp)) {
                            SentenceText(
                                s, LineMode.Current, word, annotations, fontSize, lineHeight, otherColor,
                                onWord = { w -> onWord(i, w) },
                                onWordLong = { w -> onWord(i, w) },
                            )
                            if (annotations) {
                                val (links, weak) = annotationCounts(s)
                                if (links + weak > 0) Row(Modifier.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    if (links > 0) Chip("$links 处连读", ChipKind.Violet, height = 26.dp, style = Type.chip.copy(fontSize = 12.sp))
                                    if (weak > 0) Chip("$weak 个弱读", ChipKind.Muted, height = 26.dp, style = Type.chip.copy(fontSize = 12.sp))
                                }
                            }
                        }
                    } else {
                        val small = fontSize * 19f / 24f
                        SentenceText(
                            // 非当前句不画标注（设计稿：只在当前句显示 ‿ / 弱读 / 点线）
                            s, if (s.ready) LineMode.Other else LineMode.Pending, null, false, small, lineHeight, otherColor,
                            onSentence = if (s.ready) ({ repo.seekSentence(i); following = true }) else null,
                            onWordLong = if (s.ready) ({ w -> onWord(i, w) }) else null,
                        )
                    }
                }
            }
        }
        AnimatedVisibility(
            !following && current != null,
            enter = fadeIn(fadeSpec(reduce)), exit = fadeOut(fadeSpec(reduce)),
            modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 262.dp),
        ) {
            Chip("回到当前句", ChipKind.Accent, height = 36.dp, hPad = 14.dp) { following = true; resumeTick++ }
        }
    }
}
