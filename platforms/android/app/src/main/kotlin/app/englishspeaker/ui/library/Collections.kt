package app.englishspeaker.ui.library

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import app.englishspeaker.core.ArticleCollection
import app.englishspeaker.core.PlayerStatus
import app.englishspeaker.data.SpeakerRepository
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.CollectionPrompt
import app.englishspeaker.ui.TabChrome
import app.englishspeaker.ui.components.Chip
import app.englishspeaker.ui.components.ChipKind
import app.englishspeaker.ui.components.IconBtn
import app.englishspeaker.ui.components.WaveCover
import app.englishspeaker.ui.components.pressable
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Type
import kotlinx.coroutines.launch
import kotlin.math.roundToInt

// 分类（同歌单）：资料库里的分类卡片、分类页、命名 / 删除对话框。一篇文章可以同时在多个分类里。

/** 分类封面：4 篇及以上拼成 2×2，1–3 篇用第一篇的封面，空分类用分类名生成的 violet 声波。 */
@Composable
fun CollectionCover(repo: SpeakerRepository, c: ArticleCollection, size: Int) {
    val p = LocalPalette.current
    val items = repo.articlesIn(c.id)
    when {
        items.size >= 4 -> {
            val gap = maxOf(2, (size / 40.0).roundToInt())
            val half = (size - gap) / 2
            Column(
                Modifier.size(size.dp).clip(RoundedCornerShape((size / 4.5).roundToInt().dp)),
                verticalArrangement = Arrangement.spacedBy(gap.dp),
            ) {
                for (r in 0..1) Row(horizontalArrangement = Arrangement.spacedBy(gap.dp)) {
                    for (col in 0..1) {
                        val i = r * 2 + col
                        WaveCover(items[i].title, half, if (i == 1 || i == 2) p.violet else p.accent)
                    }
                }
            }
        }
        items.isNotEmpty() -> WaveCover(items.first().title, size, p.accent)
        else -> WaveCover(c.name, size, p.violet)
    }
}

/** 资料库“全部”下的分类：新建分类 + 每个分类一张卡片，横向滚动。长按卡片：播放全部 / 重命名 / 删除。 */
@Composable
fun CollectionShelf(vm: AppViewModel) {
    val p = LocalPalette.current
    val repo = vm.repo
    val collections by repo.collections.collectAsState()
    repo.articles.collectAsState().value // 文章变化时刷新封面
    val cover = 132
    Column(Modifier.padding(top = 20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("分类", style = Type.section, color = p.t1)
        LazyRow(
            // 左右贴边滚动（突破列表的 20dp 页边距），首尾留出页面边距
            modifier = Modifier.bleed(20.dp),
            contentPadding = PaddingValues(horizontal = 20.dp),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            item(key = "new") {
                ShelfCard("新建分类", "像歌单一样整理", cover, Modifier.testTag("collection-new"), onClick = {
                    vm.collectionPrompt = CollectionPrompt.Create()
                }) {
                    val r = (cover / 4.5).roundToInt().dp
                    Box(
                        Modifier.size(cover.dp)
                            .background(p.surface, RoundedCornerShape(r))
                            .drawBehind {
                                drawRoundRect(
                                    p.waveB, cornerRadius = CornerRadius(r.toPx()),
                                    style = Stroke(1.5.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(5.dp.toPx(), 4.dp.toPx()))),
                                )
                            },
                        contentAlignment = Alignment.Center,
                    ) { Icon(Icons.line("plus", 2f), null, Modifier.size(30.dp), tint = p.t2) }
                }
            }
            items(collections, key = { it.id }) { c ->
                var menu by remember { mutableStateOf(false) }
                val n = c.articleIds.size
                Box {
                    ShelfCard(
                        c.name, if (n > 0) "$n 篇" else "空分类", cover,
                        Modifier.testTag("collection-${c.name}").semantics { contentDescription = "${c.name}，$n 篇" },
                        onLongClick = { menu = true },
                        onClick = { vm.openCollection = c.id },
                    ) { CollectionCover(repo, c, cover) }
                    CollectionMenu(vm, c, menu) { menu = false }
                }
            }
        }
    }
}

@Composable
private fun ShelfCard(
    name: String,
    detail: String,
    width: Int,
    modifier: Modifier = Modifier,
    onLongClick: (() -> Unit)? = null,
    onClick: () -> Unit,
    cover: @Composable () -> Unit,
) {
    val p = LocalPalette.current
    Column(modifier.width(width.dp).pressable(onLongClick = onLongClick, onClick = onClick)) {
        cover()
        Text(name, style = Type.rowTitle.copy(fontSize = Type.secondary.fontSize * 14 / 13), color = p.t1,
            maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 8.dp))
        Text(detail, style = Type.secondary.copy(fontSize = Type.secondary.fontSize * 12 / 13), color = p.t2,
            modifier = Modifier.padding(top = 2.dp))
    }
}

/** 分类的更多操作（长按卡片 / 分类页 ⋯）。 */
@Composable
fun CollectionMenu(vm: AppViewModel, c: ArticleCollection, expanded: Boolean, onDismiss: () -> Unit) {
    val p = LocalPalette.current
    DropdownMenu(expanded, onDismiss, containerColor = p.surface, shape = RoundedCornerShape(16.dp)) {
        DropdownMenuItem(
            text = { Text("播放全部", style = Type.body, color = if (c.articleIds.isEmpty()) p.t3 else p.t1) },
            enabled = c.articleIds.isNotEmpty(),
            onClick = { playCollection(vm, c.id); onDismiss() },
        )
        DropdownMenuItem(text = { Text("重命名", style = Type.body, color = p.t1) }, onClick = {
            vm.collectionPrompt = CollectionPrompt.Rename(c.id); onDismiss()
        })
        HorizontalDivider(color = p.line)
        DropdownMenuItem(text = { Text("删除分类", style = Type.body, color = p.dangerText) }, onClick = {
            vm.confirmDeleteCollection = c.id; onDismiss()
        })
    }
}

/** 播放分类：从第一篇开始，其余依次排进待播清单。 */
private fun playCollection(vm: AppViewModel, id: String) {
    val ids = vm.repo.articlesIn(id).map { it.id }
    ids.firstOrNull()?.let { vm.repo.play(it, ids) }
}

/**
 * 分类页：从右侧推入（盖在资料库上），左上角 / 系统返回 / 左边缘右滑返回。
 * 大封面 + 名称 + 篇数 / 时长 · 播放全部 · ⋯ · 文章列表。
 */
@Composable
fun CollectionScreen(vm: AppViewModel, c: ArticleCollection, chrome: TabChrome, offsetX: Float, onDrag: (Float) -> Unit, onDragEnd: (Boolean) -> Unit) {
    val p = LocalPalette.current
    val repo = vm.repo
    repo.articles.collectAsState().value
    val player by repo.player.collectAsState()
    val items = repo.articlesIn(c.id)
    val playing = player.status == PlayerStatus.PLAYING && items.any { it.id == player.articleId }
    val totalMs = items.sumOf { it.durationMs.toLong() }
    val state = rememberLazyListState()
    ReportScroll(state, chrome)
    var menu by remember { mutableStateOf(false) }
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    // 播放页打开时让它先处理返回（后组合的 BackHandler 优先，这里不能抢在播放页前面）
    BackHandler(enabled = !vm.playerOpen) { vm.openCollection = null }
    Box(Modifier.fillMaxSize().offset { IntOffset(offsetX.roundToInt(), 0) }.background(p.bg)) {
        LazyColumn(
            state = state,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = top + 14.dp, bottom = chrome.bottomPadding),
        ) {
            item {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    IconBtn(Icons.line("down", 2.2f), "返回资料库", fill = true, modifier = Modifier.graphicsLayer { rotationZ = 90f }) {
                        vm.openCollection = null
                    }
                    Spacer(Modifier.weight(1f))
                    Box {
                        IconBtn(Icons.more, "更多操作", fill = true) { menu = true }
                        CollectionMenu(vm, c, menu) { menu = false }
                    }
                }
            }
            item {
                Column(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                    CollectionCover(repo, c, 200)
                    Text(c.name, style = Type.largeTitle.copy(fontSize = Type.largeTitle.fontSize * 26 / 34), color = p.t1,
                        textAlign = TextAlign.Center, maxLines = 2, overflow = TextOverflow.Ellipsis, modifier = Modifier.padding(top = 14.dp))
                    Text(
                        if (items.isEmpty()) "空分类" else "${items.size} 篇 · ${maxOf(1, (totalMs / 60000.0).roundToInt())} 分钟",
                        style = Type.secondary, color = p.t2, modifier = Modifier.padding(top = 4.dp),
                    )
                    Row(
                        Modifier.padding(top = 14.dp).height(48.dp).width(180.dp).clip(CircleShape)
                            .background(if (items.isEmpty()) p.accent.copy(alpha = 0.45f) else p.accent)
                            .pressable(enabled = items.isNotEmpty()) { if (playing) repo.pause() else playCollection(vm, c.id) },
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.Center,
                    ) {
                        Icon(if (playing) Icons.pause else Icons.play, null, Modifier.size(16.dp), tint = p.accentInk)
                        Spacer(Modifier.width(8.dp))
                        Text(if (playing) "暂停" else "播放全部", style = Type.rowTitle, color = p.accentInk)
                    }
                }
            }
            if (items.isEmpty()) item {
                Text(
                    "在文章的 ⋯ 菜单里选“添加到分类”，就能把文章放进来。一篇文章可以放进多个分类。",
                    style = Type.secondary, color = p.t2, textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth().padding(vertical = 28.dp),
                )
            }
            item { Spacer(Modifier.height(12.dp)) }
            itemsIndexed(items, key = { _, a -> a.id }) { i, a ->
                ArticleRow(
                    repo, a, i, onClick = { vm.openArticle(a, items.map { it.id }) },
                    inCollection = c.id, onNewCollection = { vm.collectionPrompt = CollectionPrompt.Create(it) },
                )
            }
        }
        // 左边缘右滑返回（同系统返回手势的视觉）
        val scope = rememberCoroutineScope()
        Box(
            Modifier.fillMaxHeight().width(20.dp).pointerInput(Unit) {
                var dx = 0f
                detectHorizontalDragGestures(
                    onDragStart = { dx = 0f },
                    onDragEnd = { scope.launch { onDragEnd(dx > 100.dp.toPx()) } },
                    onDragCancel = { onDragEnd(false) },
                ) { _, d -> dx = maxOf(0f, dx + d); onDrag(dx) }
            }
        )
    }
}

/** 向左右各撑出 [d]（抵消外层列表的页边距），用于贴边横向滚动的内容。 */
private fun Modifier.bleed(d: androidx.compose.ui.unit.Dp) = layout { m, c ->
    val extra = (d * 2).roundToPx()
    val pl = m.measure(c.copy(minWidth = c.maxWidth + extra, maxWidth = c.maxWidth + extra))
    layout(c.maxWidth, pl.height) { pl.place(-extra / 2, 0) }
}

/** 分类的命名（新建 / 重命名）与删除确认，挂在资料库上。 */
@Composable
fun CollectionDialogs(vm: AppViewModel) {
    val p = LocalPalette.current
    val repo = vm.repo
    vm.collectionPrompt?.let { prompt ->
        val renaming = prompt as? CollectionPrompt.Rename
        var text by remember(prompt) { mutableStateOf(renaming?.let { repo.collection(it.id)?.name } ?: "") }
        var error by remember(prompt) { mutableStateOf<String?>(null) }
        val focus = remember { FocusRequester() }
        LaunchedEffect(prompt) { focus.requestFocus() }
        val submit = {
            error = repo.collectionNameError(text, renaming?.id)
            if (error == null) {
                val name = text.trim()
                when (prompt) {
                    is CollectionPrompt.Create -> repo.createCollection(name, prompt.articleId)
                    is CollectionPrompt.Rename -> repo.renameCollection(prompt.id, name)
                }
                vm.collectionPrompt = null
            }
        }
        AlertDialog(
            onDismissRequest = { vm.collectionPrompt = null },
            containerColor = p.surface,
            title = { Text(if (renaming != null) "重命名分类" else "新建分类", style = Type.sheetTitle, color = p.t1) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    val hint = when {
                        renaming != null -> null
                        (prompt as CollectionPrompt.Create).articleId != null -> "建好后会把这篇文章加进去。"
                        else -> "把文章整理到分类里，像歌单一样一起听。"
                    }
                    hint?.let { Text(it, style = Type.bodyMedium, color = p.t2) }
                    val shape = RoundedCornerShape(14.dp)
                    Box(
                        Modifier.fillMaxWidth().height(48.dp).background(p.card, shape)
                            .border(if (error != null) 1.5.dp else 1.dp, if (error != null) p.dangerText else p.line, shape)
                            .padding(horizontal = 14.dp),
                        contentAlignment = Alignment.CenterStart,
                    ) {
                        if (text.isEmpty()) Text("分类名称，如“新闻”“睡前故事”", style = Type.body, color = p.t3)
                        BasicTextField(
                            text, { text = it.take(40); error = null },
                            textStyle = Type.body.copy(color = p.t1),
                            cursorBrush = SolidColor(p.accent),
                            singleLine = true,
                            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                            keyboardActions = KeyboardActions(onDone = { submit() }),
                            modifier = Modifier.fillMaxWidth().focusRequester(focus).testTag("collection-name"),
                        )
                    }
                    Text(error ?: "", style = Type.secondary, color = p.dangerText)
                }
            },
            confirmButton = {
                Chip(if (renaming != null) "保存" else "创建", ChipKind.Accent, height = 40.dp, hPad = 16.dp) { submit() }
            },
            dismissButton = { Chip("取消", ChipKind.Neutral, height = 40.dp, hPad = 16.dp) { vm.collectionPrompt = null } },
        )
    }
    vm.confirmDeleteCollection?.let { id ->
        val c = repo.collection(id)
        AlertDialog(
            onDismissRequest = { vm.confirmDeleteCollection = null },
            containerColor = p.surface,
            title = { Text(c?.let { "删除“${it.name}”？" } ?: "删除分类？", style = Type.sheetTitle, color = p.t1) },
            text = { Text("其中的文章不会被删除，只是不再属于这个分类。", style = Type.bodyMedium, color = p.t2) },
            confirmButton = {
                Chip("删除", ChipKind.Danger, height = 40.dp, hPad = 16.dp) {
                    if (vm.openCollection == id) vm.openCollection = null
                    repo.deleteCollection(id)
                    vm.confirmDeleteCollection = null
                }
            },
            dismissButton = { Chip("取消", ChipKind.Neutral, height = 40.dp, hPad = 16.dp) { vm.confirmDeleteCollection = null } },
        )
    }
}
