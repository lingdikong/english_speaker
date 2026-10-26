package app.englishspeaker.ui.search

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
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
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.englishspeaker.core.ArticleSummary
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.TabChrome
import app.englishspeaker.ui.components.IconBtn
import app.englishspeaker.ui.components.LargeTitle
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.library.ArticleRow
import app.englishspeaker.ui.library.ReportScroll
import app.englishspeaker.ui.library.openArticle
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Type
import kotlinx.coroutines.delay

/** 搜索：搜索框（card 底，圆角 14），按标题与正文过滤；命中词用 accent-soft 胶囊标出。 */
@Composable
fun SearchScreen(vm: AppViewModel, chrome: TabChrome) {
    val p = LocalPalette.current
    val repo = vm.repo
    val articles by repo.articles.collectAsState()
    var results by remember { mutableStateOf<List<Pair<ArticleSummary, String?>>>(emptyList()) }
    val query = vm.searchQuery
    LaunchedEffect(query, articles) {
        delay(150)
        results = repo.search(query)
    }
    val state = rememberLazyListState()
    ReportScroll(state, chrome)
    val top = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    LazyColumn(
        state = state,
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, top = top + 14.dp, bottom = chrome.bottomPadding),
    ) {
        item { LargeTitle("搜索") }
        item {
            val shape = RoundedCornerShape(14.dp)
            Row(
                Modifier
                    .padding(top = 20.dp, bottom = 8.dp)
                    .fillMaxWidth()
                    .height(48.dp)
                    .background(p.card, shape)
                    .border(1.dp, p.line, shape)
                    .padding(start = 14.dp, end = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(Icons.line("search", 2f), null, Modifier.size(18.dp), tint = p.t2)
                Spacer(Modifier.width(10.dp))
                Box(Modifier.weight(1f)) {
                    if (query.isEmpty()) Text("搜索文章标题与原文", style = Type.body, color = p.t2)
                    BasicTextField(
                        query, { vm.searchQuery = it },
                        textStyle = Type.body.copy(color = p.t1),
                        cursorBrush = SolidColor(p.accent),
                        singleLine = true,
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
                if (query.isNotEmpty()) IconBtn(Icons.line("close", 2f), "清除", iconSize = 16.dp, tint = p.t2) { vm.searchQuery = "" }
            }
        }
        if (query.isBlank()) {
            item { Text("输入关键词，按标题和正文查找。", style = Type.secondary, color = p.t2, modifier = Modifier.padding(vertical = 16.dp)) }
        } else if (results.isEmpty()) {
            item { Text("没有找到“$query”", style = Type.secondary, color = p.t2, modifier = Modifier.padding(vertical = 16.dp)) }
        } else {
            item { Text("${results.size} 篇文章", style = Type.section, color = p.t1, modifier = Modifier.padding(top = 12.dp, bottom = 4.dp)) }
        }
        itemsIndexed(results, key = { _, r -> r.first.id }) { i, (a, snippet) ->
            ArticleRow(
                repo, a, i, onClick = { vm.openArticle(a, results.map { it.first.id }) }, query = query, snippet = snippet,
                onNewCollection = { vm.collectionPrompt = app.englishspeaker.ui.CollectionPrompt.Create(it) },
            )
        }
    }
}
