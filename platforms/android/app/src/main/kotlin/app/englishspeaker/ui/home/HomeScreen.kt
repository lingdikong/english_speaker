package app.englishspeaker.ui.home

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
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
import app.englishspeaker.core.PlayerStatus
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.TabChrome
import app.englishspeaker.ui.components.IconBtn
import app.englishspeaker.ui.components.LargeTitle
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.library.ArticleRow
import app.englishspeaker.ui.library.ContinueCard
import app.englishspeaker.ui.library.EmptyState
import app.englishspeaker.ui.library.ReportScroll
import app.englishspeaker.ui.library.openArticle
import app.englishspeaker.ui.library.playOrToggle
import app.englishspeaker.ui.library.rememberContinue
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Type

/** 首页：大标题；继续收听；最近添加。无内容时显示空状态。 */
@Composable
fun HomeScreen(vm: AppViewModel, chrome: TabChrome) {
    val p = LocalPalette.current
    val repo = vm.repo
    val articles by repo.articles.collectAsState()
    val ready by repo.ready.collectAsState()
    val player by repo.player.collectAsState()
    val state = rememberLazyListState()
    ReportScroll(state, chrome)
    val cont = rememberContinue(vm)
    val recent = articles.sortedByDescending { it.createdAtMs }
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    LazyColumn(
        state = state,
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = top + 14.dp, bottom = chrome.bottomPadding),
    ) {
        item {
            LargeTitle("首页") {
                IconBtn(Icons.line("plus", 2.2f), "导入文章", background = p.accent, tint = p.accentInk) { vm.showImport = true }
            }
        }
        if (ready && articles.isEmpty()) {
            item { EmptyState { vm.showImport = true } }
            return@LazyColumn
        }
        if (cont != null) {
            item {
                Box(Modifier.padding(top = 20.dp)) {
                    ContinueCard(
                        cont.article.title, cont.detail, cont.progress,
                        playing = cont.isCurrent && player.status == PlayerStatus.PLAYING,
                        buffering = cont.isCurrent && player.status == PlayerStatus.BUFFERING,
                        onPlay = { vm.playOrToggle(cont.article, recent.map { it.id }) },
                        onOpen = { vm.openArticle(cont.article, recent.map { it.id }) },
                    )
                }
            }
        }
        item { Text("最近添加", style = Type.section, color = p.t1, modifier = Modifier.padding(top = 20.dp, bottom = 4.dp)) }
        itemsIndexed(recent, key = { _, a -> a.id }) { i, a ->
            ArticleRow(
                repo, a, i, onClick = { vm.openArticle(a, recent.map { it.id }) },
                onNewCollection = { vm.collectionPrompt = app.englishspeaker.ui.CollectionPrompt.Create(it) },
            )
        }
    }
}
