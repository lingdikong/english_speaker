package app.englishspeaker.ui

import android.app.Activity
import androidx.activity.compose.BackHandler
import androidx.compose.animation.Crossfade
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.graphicsLayer
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
import app.englishspeaker.ui.player.PlayerMorph
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
    // 播放页收起进度：0 = 全屏，1 = 已收进底部小球。放在这里而不是播放页内部，
    // 是为了让它跨“打开/关闭”存活，并且底层界面能跟着一起放大回来。
    val collapse = remember { Animatable(1f) }
    val collapseVal by collapse.asState()
    BackHandler(vm.playerOpen) { vm.requestCollapse() }

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
                    // 底层界面随播放页收起同步放大回来（与 iOS RootView 一致）：
                    // 播放页缩小时后面露出的不是黑底，而是正常界面
                    .graphicsLayer {
                        val s = 0.94f + 0.06f * (1f - collapseVal)
                        scaleX = s
                        scaleY = s
                    }
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

            // 播放页不再用 AnimatedVisibility 滑入滑出：收起/展开由 PlayerMorph 的
            // “整页缩回底部小球”统一负责（与 iOS 一致），滑入动画会和它打架。
            if (vm.playerOpen && current != null) {
                PlayerMorph(
                    vm = vm,
                    collapse = collapse,
                    barState = vm.bar,
                    base = base,
                    ballSize = Bar.height,
                    onDismiss = { vm.playerOpen = false },
                ) {
                    PlayerScreen(vm, current)
                }
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
