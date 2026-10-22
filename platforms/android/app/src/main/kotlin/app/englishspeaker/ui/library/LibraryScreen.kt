package app.englishspeaker.ui.library

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.englishspeaker.core.AudioStatus
import app.englishspeaker.core.PlayerStatus
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.Tab
import app.englishspeaker.ui.TabChrome
import app.englishspeaker.ui.components.Chip
import app.englishspeaker.ui.components.ChipKind
import app.englishspeaker.ui.components.IconBtn
import app.englishspeaker.ui.components.LargeTitle
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Type

private fun AudioStatus.converting() = this == AudioStatus.QUEUED || this == AudioStatus.SYNTHESIZING || this == AudioStatus.PARTIAL

/** 资料库：大标题 + 搜索 / 导入；筛选 chip；继续收听；全部文章。 */
@Composable
fun LibraryScreen(vm: AppViewModel, chrome: TabChrome) {
    val p = LocalPalette.current
    val repo = vm.repo
    val articles by repo.articles.collectAsState()
    val player by repo.player.collectAsState()
    val state = rememberLazyListState()
    ReportScroll(state, chrome)
    val cont = rememberContinue(vm)
    val filtered = when (vm.libraryFilter) {
        1 -> articles.filter { it.status.converting() }
        2 -> articles.filter { it.pinned }
        3 -> articles.filter { it.status == AudioStatus.NOT_CACHED || it.status == AudioStatus.FAILED }
        else -> articles
    }
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    LazyColumn(
        state = state,
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = top + 14.dp, bottom = chrome.bottomPadding),
    ) {
        item {
            LargeTitle("资料库") {
                IconBtn(Icons.search, "搜索", fill = true) { vm.selectTab(Tab.Search) }
                IconBtn(Icons.line("plus", 2.2f), "导入文章", background = p.accent, tint = p.accentInk) { vm.showImport = true }
            }
        }
        item {
            val conv = articles.count { it.status.converting() }
            val labels = listOf("全部 ${articles.size}", "转换中 $conv", "已保存", "未转换")
            Row(Modifier.padding(top = 20.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                labels.forEachIndexed { i, l ->
                    Chip(
                        l, if (i == vm.libraryFilter) ChipKind.Accent else ChipKind.Muted,
                        height = 34.dp, style = Type.chip.copy(fontSize = 14.sp),
                    ) { vm.libraryFilter = i }
                }
            }
        }
        if (cont != null && vm.libraryFilter == 0) {
            item {
                androidx.compose.foundation.layout.Box(Modifier.padding(top = 20.dp)) {
                    ContinueCard(
                        cont.article.title, cont.detail, cont.progress,
                        playing = cont.isCurrent && player.status == PlayerStatus.PLAYING,
                        buffering = cont.isCurrent && player.status == PlayerStatus.BUFFERING,
                        onPlay = { vm.playOrToggle(cont.article, articles.map { it.id }) },
                        onOpen = { vm.openArticle(cont.article, articles.map { it.id }) },
                    )
                }
            }
        }
        item {
            Text(
                when (vm.libraryFilter) { 1 -> "转换中"; 2 -> "已保存"; 3 -> "未转换"; else -> "全部文章" },
                style = Type.section, color = p.t1, modifier = Modifier.padding(top = 20.dp, bottom = 4.dp),
            )
        }
        if (articles.isEmpty()) {
            item { EmptyState { vm.showImport = true } }
        } else if (filtered.isEmpty()) {
            item { Text("没有符合条件的文章", style = Type.secondary, color = p.t2, modifier = Modifier.padding(vertical = 24.dp)) }
        }
        itemsIndexed(filtered, key = { _, a -> a.id }) { i, a ->
            ArticleRow(repo, a, i, onClick = { vm.openArticle(a, filtered.map { it.id }) })
        }
    }
}
