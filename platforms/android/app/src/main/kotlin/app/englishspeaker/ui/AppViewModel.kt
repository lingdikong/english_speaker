package app.englishspeaker.ui

import android.os.Bundle
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.englishspeaker.SpeakerApp
import app.englishspeaker.core.PlayerStatus
import app.englishspeaker.data.ThemeMode
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

enum class Tab { Home, Library, Search, Settings }

/** 底部栏三态（UI_SPEC §4.2）。 */
enum class BarState { A, B, C }

/** 分类命名对话框的用途：新建（给了 articleId 时建好后把这篇加进去）/ 重命名。 */
sealed interface CollectionPrompt {
    data class Create(val articleId: String? = null) : CollectionPrompt
    data class Rename(val id: String) : CollectionPrompt
}

/** 点词读音面板的目标：句下标 + 词下标。 */
data class WordTarget(val sentence: Int, val word: Int)

/** 纯 UI 状态；业务状态全部在 [app.englishspeaker.data.SpeakerRepository]。 */
class AppViewModel : ViewModel() {
    val repo = SpeakerApp.repo
    val settings = SpeakerApp.settings

    var tab by mutableStateOf(Tab.Library)
        private set
    var playerOpen by mutableStateOf(false)
    /**
     * 收起请求计数（递增即触发一次收起动画）。
     * 播放页不能靠直接置 [playerOpen] = false 关闭——那样会瞬间消失，
     * 与 iOS 一致要先播放「整页缩回底部小球」的动画，动画结束才真正移除。
     */
    var collapseRequest by mutableIntStateOf(0)

    fun requestCollapse() { collapseRequest++ }
    var showImport by mutableStateOf(false)
    var showUpNext by mutableStateOf(false)
    var showReading by mutableStateOf(false)
    var word by mutableStateOf<WordTarget?>(null)
    var searchQuery by mutableStateOf("")
    /** 截图/端到端测试用：导入 Sheet 的预填内容（标题, 正文）。 */
    var importPrefill by mutableStateOf<Pair<String, String>?>(null)
    var importPlayNow by mutableStateOf(true)
    var libraryFilter by mutableIntStateOf(0)
    /** 资料库里打开的分类页。 */
    var openCollection by mutableStateOf<String?>(null)
    /** 分类命名对话框；删除分类的确认。 */
    var collectionPrompt by mutableStateOf<CollectionPrompt?>(null)
    var confirmDeleteCollection by mutableStateOf<String?>(null)
    /** 截图用：强制专注模式。 */
    var forceFocus by mutableStateOf(false)

    // —— 底部栏状态机 ——
    var bar by mutableStateOf(BarState.A)
        private set
    private val scrollY = mutableStateMapOf<Tab, Int>()

    /** 每个 Tab 自己的滚动容器上报滚动位置（px）。 */
    fun onScroll(t: Tab, y: Int, density: Float) {
        scrollY[t] = y
        if (t != tab) return
        val dp = y / density
        when {
            dp < 4f -> bar = BarState.A
            dp > 24f && bar == BarState.A -> bar = BarState.B
            // C 下继续滚动保持 C
        }
    }

    fun selectTab(t: Tab) {
        tab = t
        // 新 Tab 在顶部 → A；否则保持一行（C 时用户刚点开导航，保持 C）
        if ((scrollY[t] ?: 0) == 0) bar = BarState.A else if (bar == BarState.A) bar = BarState.B
    }

    /** 点左侧导航小球 → C（没有播放条时 C 即全宽导航，滚动时同样保持展开）。 */
    fun expandTabs() { bar = BarState.C }

    /** C 下点右侧播放小球 → B。 */
    fun expandPlayer() { bar = BarState.B }

    fun forceBar(s: BarState) { bar = s }

    /**
     * 调试/截图用的启动参数（adb shell am start -n app.englishspeaker/.MainActivity --es screen player …）：
     * - screen：home | library | search | settings | import | player | upnext | reading | word | focus
     * - theme：system | light | dark
     * - bar：A | B | C（强制底部栏状态）
     * - autoplay / article：播放资料库第 N 篇；seekSentence：跳到第 N 句；pauseAfter：N 毫秒后暂停
     * - annotations：是否显示跟读标注；query：搜索词；wordIndex：点词面板的词下标（当前句）
     * - demoCollections：预置两个分类“晨间”（前 4 篇）、“睡前故事”（第 2 篇，与晨间重叠）；openCollection：打开名为该值的分类页
     */
    fun applyLaunchArgs(extras: Bundle?) {
        extras ?: return
        when (extras.getString("theme")) {
            "light" -> settings.setTheme(ThemeMode.Light)
            "dark" -> settings.setTheme(ThemeMode.Dark)
            "system" -> settings.setTheme(ThemeMode.System)
        }
        val screen = extras.getString("screen")
        when (screen) {
            "home" -> { selectTab(Tab.Home); playerOpen = false }
            "library" -> { selectTab(Tab.Library); playerOpen = false }
            "search" -> { selectTab(Tab.Search); playerOpen = false }
            "settings" -> { selectTab(Tab.Settings); playerOpen = false }
            "import" -> {
                extras.getString("importBody")?.let { importPrefill = (extras.getString("importTitle") ?: "") to it }
                importPlayNow = extras.getBoolean("importPlay", true)
                showImport = true
            }
            "player", "nowPlaying", "lyrics" -> playerOpen = true
            "upnext", "queue" -> { playerOpen = true; showUpNext = true }
            "reading" -> { playerOpen = true; showReading = true }
            "focus" -> { playerOpen = true; forceFocus = true }
            "word" -> playerOpen = true
        }
        if (screen != "focus") forceFocus = false
        extras.getString("bar")?.let { b -> runCatching { BarState.valueOf(b) }.getOrNull()?.let { forceBar(it) } }
        if (extras.containsKey("annotations")) repo.setAnnotations(extras.getBoolean("annotations"))
        if (extras.containsKey("fontSize")) settings.setFontSize(extras.getInt("fontSize"))
        // 截图用：制造多种合成状态（真实核心调用）：第 1 篇删除缓存 → 未转换；第 2、3 篇换音色重新转换 → 排队中 / 转换中
        if (extras.getBoolean("demoStates", false)) viewModelScope.launch {
            repo.ready.first { it }
            val list = repo.articles.first { it.size >= 4 }
            val voices = repo.voices.value
            repo.removeDownload(list[1].id)
            listOf(list[2], list[3]).forEachIndexed { i, a ->
                val v = voices.firstOrNull { it.id != (a.voiceId ?: repo.defaultVoice.value) && (i == 0 || it.accent != voices.first().accent) } ?: voices.first()
                repo.synthesize(a.id, v.id)
            }
        }
        extras.getString("query")?.let { searchQuery = it }
        val openName = extras.getString("openCollection")
        if (extras.getBoolean("demoCollections", false) || openName != null) viewModelScope.launch {
            repo.ready.first { it }
            val list = repo.articles.first { it.size >= 2 }
            if (extras.getBoolean("demoCollections", false) && repo.collections.value.isEmpty()) {
                repo.createCollection("晨间")?.let { c -> list.take(4).forEach { repo.addToCollection(c.id, it.id) } }
                repo.createCollection("睡前故事", adding = list[1].id)
            }
            if (openName != null) {
                val c = repo.collections.first { cs -> cs.any { it.name == openName } }.first { it.name == openName }
                selectTab(Tab.Library)
                openCollection = c.id
            }
        }
        val autoplay = extras.getBoolean("autoplay", false)
        val index = extras.getInt("article", 0)
        val seekSentence = extras.getInt("seekSentence", -1)
        val pauseAfter = extras.getLong("pauseAfter", -1L)
        val wordIndex = extras.getInt("wordIndex", -1)
        if (!autoplay && seekSentence < 0 && pauseAfter < 0 && screen != "word") return
        viewModelScope.launch {
            repo.ready.first { it }
            val list = repo.articles.first { it.isNotEmpty() }
            if (autoplay) {
                val a = list.getOrNull(index) ?: list.first()
                repo.play(a.id, list.map { it.id })
            }
            if (seekSentence >= 0) {
                repo.timeline.first { t -> t?.sentences?.getOrNull(seekSentence)?.ready == true }
                repo.seekSentence(seekSentence)
            }
            if (pauseAfter >= 0) {
                repo.player.first { it.status == PlayerStatus.PLAYING }
                delay(pauseAfter)
                repo.pause()
            }
            if (screen == "word") {
                val s = repo.position.value.sentence ?: 0
                val w = if (wordIndex >= 0) wordIndex else (repo.position.value.word ?: 0)
                word = WordTarget(s, w)
            }
        }
    }
}
