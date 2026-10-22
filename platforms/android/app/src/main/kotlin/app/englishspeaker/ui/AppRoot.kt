package app.englishspeaker.ui

import android.app.Activity
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.ui.graphics.Brush
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.core.view.WindowCompat
import app.englishspeaker.core.PlayerStatus
import app.englishspeaker.ui.components.FadeStops
import app.englishspeaker.ui.components.LocalHaze
import app.englishspeaker.ui.components.fadeUnder
import app.englishspeaker.ui.components.fadeTop
import app.englishspeaker.ui.home.HomeScreen
import app.englishspeaker.ui.importer.ImportSheet
import app.englishspeaker.ui.library.LibraryScreen
import app.englishspeaker.ui.player.PlayerScreen
import app.englishspeaker.ui.player.ReadingSettingsSheet
import app.englishspeaker.ui.player.UpNextSheet
import app.englishspeaker.ui.player.WordSheet
import app.englishspeaker.ui.search.SearchScreen
import app.englishspeaker.ui.settings.SettingsScreen
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.LocalReduceMotion
import app.englishspeaker.ui.theme.fadeSpec
import dev.chrisbanes.haze.hazeSource
import dev.chrisbanes.haze.rememberHazeState

/** 每个 Tab 页拿到的布局参数。 */
class TabChrome(
    /** 列表底部留白（让最后一行能滚到玻璃之上）。 */
    val bottomPadding: Dp,
    /** 滚动位置上报 → 底部栏状态机。 */
    val onScroll: (Int) -> Unit,
)

@Composable
fun AppRoot(vm: AppViewModel) {
    val p = LocalPalette.current
    val reduce = LocalReduceMotion.current
    val haze = rememberHazeState()
    val repo = vm.repo
    val player by repo.player.collectAsState()
    val articles by repo.articles.collectAsState()
    val position by repo.position.collectAsState()
    val timeline by repo.timeline.collectAsState()
    val current = articles.firstOrNull { it.id == player.articleId }

    // 状态栏/导航栏图标跟随主题
    val view = LocalView.current
    LaunchedEffect(p.dark) {
        val window = (view.context as Activity).window
        WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = !p.dark
        WindowCompat.getInsetsController(window, view).isAppearanceLightNavigationBars = !p.dark
    }
    BackHandler(vm.playerOpen) { vm.playerOpen = false }

    // 底部离手势条 26；三键导航时放到导航栏之上
    val nav = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    val base = if (nav <= 30.dp) Bar.bottom else nav + 8.dp
    val hasPlayer = current != null
    val oneRow = !hasPlayer || vm.bar != BarState.A
    val fadeT by animateFloatAsState(if (oneRow) 1f else 0f, fadeSpec(reduce, 300), label = "fade")
    val density = androidx.compose.ui.platform.LocalDensity.current.density
    val statusTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()

    CompositionLocalProvider(LocalHaze provides haze) {
        Box(Modifier.fillMaxSize().background(p.bg)) {
            Box(
                Modifier
                    .fillMaxSize()
                    .hazeSource(haze)
                    .fadeUnder(FadeStops.m, FadeStops.m1, fadeT, extraBottom = base - Bar.bottom)
                    // 滚动内容在状态栏下渐隐：状态栏区域完全透明，其下 24dp 渐变
                    .fadeTop(statusTop, 24.dp)
            ) {
                Crossfade(vm.tab, animationSpec = tween(if (reduce) 0 else 180), label = "tab") { tab ->
                    val chrome = TabChrome(barContentPadding(hasPlayer, base)) { y -> vm.onScroll(tab, y, density) }
                    when (tab) {
                        Tab.Home -> HomeScreen(vm, chrome)
                        Tab.Library -> LibraryScreen(vm, chrome)
                        Tab.Search -> SearchScreen(vm, chrome)
                        Tab.Settings -> SettingsScreen(vm, chrome)
                    }
                }
            }

            val mini = current?.let { a ->
                val dur = player.durationMs.toLong().coerceAtLeast(1)
                val sentence = position.sentence?.let { timeline?.sentences?.getOrNull(it) }
                MiniInfo(
                    a, player.status,
                    (repo.livePosition().toFloat() / dur).coerceIn(0f, 1f),
                    sentence?.text ?: timeline?.sentences?.firstOrNull()?.text ?: repo.voiceLabel(a.voiceId),
                )
            }
            BottomBar(
                state = vm.bar, tab = vm.tab, mini = mini, base = base,
                onTab = { vm.selectTab(it) },
                onExpandTabs = { vm.expandTabs() },
                onExpandPlayer = { vm.expandPlayer() },
                onOpenPlayer = { vm.playerOpen = true },
                onToggle = { repo.toggle() },
                onNext = { repo.nextSentence() },
            )

            AnimatedVisibility(
                vm.playerOpen && current != null,
                enter = if (reduce) fadeIn(tween(0)) else slideInVertically(tween(320)) { it / 4 } + fadeIn(tween(220)),
                exit = if (reduce) fadeOut(tween(0)) else slideOutVertically(tween(260)) { it / 4 } + fadeOut(tween(200)),
            ) {
                if (current != null) PlayerScreen(vm, current)
            }

            if (vm.showImport) ImportSheet(vm) { vm.showImport = false }
            if (vm.showUpNext) UpNextSheet(vm) { vm.showUpNext = false }
            if (vm.showReading) ReadingSettingsSheet(vm) { vm.showReading = false }
            vm.word?.let { w -> WordSheet(vm, w) { vm.word = null } }
        }
    }
}

/** 是否处于“活跃”播放（播放或缓冲）。 */
fun PlayerStatus.active() = this == PlayerStatus.PLAYING || this == PlayerStatus.BUFFERING
