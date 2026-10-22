package app.englishspeaker.ui.player

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import app.englishspeaker.core.ArticleSummary
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.components.AppSheet
import app.englishspeaker.ui.components.Label
import app.englishspeaker.ui.components.SheetHeader
import app.englishspeaker.ui.components.WaveCover
import app.englishspeaker.ui.components.pressable
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.library.metaText
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Type

private val RowHeight = 72.dp

/** 待播清单：正在播放 + 待播列表（长按把手拖动排序、左滑删除）；当前播完自动播下一篇（客户端队列）。 */
@Composable
fun UpNextSheet(vm: AppViewModel, onDismiss: () -> Unit) {
    val p = LocalPalette.current
    val repo = vm.repo
    val articles by repo.articles.collectAsState()
    val upNext by repo.upNext.collectAsState()
    val player by repo.player.collectAsState()
    val current = articles.firstOrNull { it.id == player.articleId }
    val items = upNext.mapNotNull { id -> articles.firstOrNull { it.id == id } }
    var draggingId by remember { mutableStateOf<String?>(null) }
    var dragOffset by remember { mutableFloatStateOf(0f) }
    val rowPx = with(LocalDensity.current) { RowHeight.toPx() }

    AppSheet(onDismiss) { close ->
        Column(Modifier.fillMaxHeight(0.85f).padding(top = 12.dp)) {
            Box(Modifier.padding(horizontal = 20.dp)) { SheetHeader("待播清单", close) }
            LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = 32.dp)) {
                if (current != null) {
                    item {
                        Label("正在播放", Modifier.padding(start = 20.dp, top = 18.dp, bottom = 4.dp))
                        QueueRow(vm, current, highlight = true) {}
                    }
                }
                item {
                    Label("接下来", Modifier.padding(start = 20.dp, top = 18.dp, bottom = 4.dp))
                    if (items.isEmpty()) {
                        Text("没有待播的文章。在资料库里点按文章会把其后的文章加入这里。", style = Type.secondary, color = p.t2,
                            modifier = Modifier.padding(horizontal = 20.dp, vertical = 12.dp))
                    }
                }
                itemsIndexed(items, key = { _, a -> a.id }) { _, a ->
                    val dragging = draggingId == a.id
                    val state = rememberSwipeToDismissBoxState()
                    Box(
                        Modifier
                            .zIndex(if (dragging) 1f else 0f)
                            .graphicsLayer {
                                translationY = if (dragging) dragOffset else 0f
                                val s = if (dragging) 1.02f else 1f
                                scaleX = s; scaleY = s
                            }
                            .then(if (dragging) Modifier else Modifier.animateItem())
                    ) {
                        SwipeToDismissBox(
                            state = state,
                            enableDismissFromStartToEnd = false,
                            onDismiss = { if (it == SwipeToDismissBoxValue.EndToStart) repo.removeUpNext(a.id) },
                            backgroundContent = {
                                if (state.dismissDirection == SwipeToDismissBoxValue.EndToStart) Box(
                                    Modifier.fillMaxSize().background(p.dangerSoft).padding(end = 24.dp),
                                    contentAlignment = Alignment.CenterEnd,
                                ) { Text("删除", style = Type.rowTitle, color = p.dangerText) }
                            },
                        ) {
                            QueueRow(
                                vm, a, highlight = false, dragging = dragging,
                                handle = Modifier.pointerInput(a.id) {
                                    detectDragGesturesAfterLongPress(
                                        onDragStart = { draggingId = a.id; dragOffset = 0f },
                                        onDragEnd = { draggingId = null; dragOffset = 0f },
                                        onDragCancel = { draggingId = null; dragOffset = 0f },
                                    ) { change, d ->
                                        change.consume()
                                        dragOffset += d.y
                                        val i = repo.upNext.value.indexOf(a.id)
                                        if (dragOffset > rowPx / 2 && i < repo.upNext.value.size - 1) {
                                            repo.moveUpNext(i, i + 1); dragOffset -= rowPx
                                        } else if (dragOffset < -rowPx / 2 && i > 0) {
                                            repo.moveUpNext(i, i - 1); dragOffset += rowPx
                                        }
                                    }
                                },
                            ) { repo.play(a.id, upNext) }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun QueueRow(
    vm: AppViewModel,
    a: ArticleSummary,
    highlight: Boolean,
    dragging: Boolean = false,
    handle: Modifier? = null,
    onClick: () -> Unit,
) {
    val p = LocalPalette.current
    Row(
        Modifier
            .fillMaxWidth()
            .height(RowHeight)
            .background(if (dragging || highlight) p.accentSoft else p.surface)
            .pressable(onClick = onClick)
            .padding(horizontal = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        WaveCover(a.title, 48, if (highlight) p.accent else p.violet)
        Spacer(Modifier.width(12.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.Center) {
            Text(a.title, style = Type.rowTitle, color = if (highlight) p.accentText else p.t1, maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text(metaText(vm.repo, a), style = Type.secondary, color = p.t2, maxLines = 1)
        }
        if (handle != null) {
            Icon(
                Icons.line("grip", 2f), null,
                handle.size(40.dp).clip(RoundedCornerShape(10.dp)).padding(8.dp).semantics { contentDescription = "长按拖动排序" },
                tint = p.t2,
            )
        }
    }
}
