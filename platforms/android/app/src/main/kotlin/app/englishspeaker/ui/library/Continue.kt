package app.englishspeaker.ui.library

import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.snapshotFlow
import app.englishspeaker.core.ArticleSummary
import app.englishspeaker.core.PlayerStatus
import app.englishspeaker.ui.AppViewModel
import app.englishspeaker.ui.TabChrome
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlin.math.ceil
import kotlin.math.roundToInt

class ContinueInfo(val article: ArticleSummary, val detail: String, val progress: Float, val isCurrent: Boolean)

/**
 * “继续收听”：正在播放 / 最近播放的文章，否则取有进度的最近一篇。
 * “第 x/y 句 · 剩余 N 分钟”：当前文章用实时位置；其它文章按已播比例估算句子。
 */
@Composable
fun rememberContinue(vm: AppViewModel): ContinueInfo? {
    val repo = vm.repo
    val articles by repo.articles.collectAsState()
    val player by repo.player.collectAsState()
    val pos by repo.position.collectAsState()
    val timeline by repo.timeline.collectAsState()
    val cur = articles.firstOrNull { it.id == player.articleId }
    val a = cur ?: articles.filter { (it.progressMs ?: 0u) > 0u }.maxByOrNull { it.createdAtMs } ?: return null
    val total = (timeline?.sentences?.size?.takeIf { cur != null } ?: a.totalSentences.toInt()).coerceAtLeast(1)
    val dur = (if (cur != null) player.durationMs else a.durationMs).toLong().coerceAtLeast(1)
    val at = if (cur != null) pos.positionMs else (a.progressMs ?: 0u).toLong()
    val frac = (at.toFloat() / dur).coerceIn(0f, 1f)
    val sentence = if (cur != null) (pos.sentence ?: 0) + 1 else ceil(frac * total).toInt().coerceIn(1, total)
    val rate = if (cur != null) player.rate.coerceAtLeast(0.25f) else 1f
    val remainMin = maxOf(1, (((dur - at).coerceAtLeast(0) / rate) / 60000f).roundToInt())
    val approx = if (cur != null && !player.durationFinal) "约 " else ""
    return ContinueInfo(a, "第 $sentence/$total 句 · 剩余 $approx$remainMin 分钟", frac, cur != null)
}

/** 播放一篇（或切换当前篇的播放/暂停）。 */
fun AppViewModel.playOrToggle(a: ArticleSummary, queue: List<String>) {
    val ps = repo.player.value
    if (ps.articleId == a.id && ps.status != PlayerStatus.IDLE) repo.toggle() else repo.play(a.id, queue)
}

/** 点按列表行：播放并打开播放页（当前篇直接打开）。 */
fun AppViewModel.openArticle(a: ArticleSummary, queue: List<String>) {
    if (repo.player.value.articleId != a.id) repo.play(a.id, queue)
    playerOpen = true
}

/** 把 LazyColumn 的滚动位置上报给底部栏状态机。 */
@Composable
fun ReportScroll(state: LazyListState, chrome: TabChrome) {
    LaunchedEffect(state) {
        snapshotFlow { if (state.firstVisibleItemIndex > 0) 100_000 else state.firstVisibleItemScrollOffset }
            .distinctUntilChanged()
            .collect { chrome.onScroll(it) }
    }
}
