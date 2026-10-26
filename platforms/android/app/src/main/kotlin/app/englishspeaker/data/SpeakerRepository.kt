package app.englishspeaker.data

import android.content.Context
import android.os.SystemClock
import android.util.Log
import app.englishspeaker.audio.AudioTrackSink
import app.englishspeaker.core.Accent
import app.englishspeaker.core.ArticleCollection
import app.englishspeaker.core.ArticleSummary
import app.englishspeaker.core.CacheStats
import app.englishspeaker.core.DictEntry
import app.englishspeaker.core.GlossaryTerm
import app.englishspeaker.core.SentenceAnalysis
import app.englishspeaker.core.SentenceTranslation
import app.englishspeaker.core.TranslationEngine
import app.englishspeaker.core.TranslatorStatus
import app.englishspeaker.core.VocabEntry
import app.englishspeaker.core.WordInsight
import app.englishspeaker.core.PlayerState
import app.englishspeaker.core.PlayerStatus
import app.englishspeaker.core.SourceKind
import app.englishspeaker.core.SpeakerCore
import app.englishspeaker.core.SpeakerEvent
import app.englishspeaker.core.SpeakerListener
import app.englishspeaker.core.Timeline
import app.englishspeaker.core.Voice
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

/** 播放位置（来自 PlayerPosition 事件），`atMs` 用于两次事件之间插值。 */
data class Position(
    val positionMs: Long = 0,
    val bufferedMs: Long = 0,
    val sentence: Int? = null,
    val word: Int? = null,
    val atMs: Long = 0,
)

/** 智能解析（举一反三 / 句子解析）的请求状态。 */
data class AiResult<T>(val loading: Boolean = false, val value: T? = null, val error: String? = null)

/**
 * 唯一的数据源：持有 [SpeakerCore]，把核心事件切回主线程转成 StateFlow。
 * 业务状态全部来自核心层；客户端只额外维护待播清单（核心层暂无队列 API）。
 */
class SpeakerRepository(private val context: Context) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val prefs = context.getSharedPreferences("app", Context.MODE_PRIVATE)
    private var core: SpeakerCore? = null

    private val _ready = MutableStateFlow(false)
    val ready: StateFlow<Boolean> = _ready
    private val _articles = MutableStateFlow<List<ArticleSummary>>(emptyList())
    val articles: StateFlow<List<ArticleSummary>> = _articles
    /** 分类（同歌单）：一篇文章可以同时在多个分类里；各自的 articleIds 按加入先后。 */
    private val _collections = MutableStateFlow<List<ArticleCollection>>(emptyList())
    val collections: StateFlow<List<ArticleCollection>> = _collections
    private val _player = MutableStateFlow(
        PlayerState(null, PlayerStatus.IDLE, 0u, 0u, 0u, false, 1f)
    )
    val player: StateFlow<PlayerState> = _player
    private val _position = MutableStateFlow(Position())
    val position: StateFlow<Position> = _position
    /** 当前文章的时间轴（全部句子；已合成的带时间与跟读标注）。 */
    private val _timeline = MutableStateFlow<Timeline?>(null)
    val timeline: StateFlow<Timeline?> = _timeline
    private val _upNext = MutableStateFlow<List<String>>(emptyList())
    val upNext: StateFlow<List<String>> = _upNext
    private val _voices = MutableStateFlow<List<Voice>>(emptyList())
    val voices: StateFlow<List<Voice>> = _voices
    private val _cache = MutableStateFlow<CacheStats?>(null)
    val cache: StateFlow<CacheStats?> = _cache
    private val _showAnnotations = MutableStateFlow(prefs.getBoolean("annotations", true))
    val showAnnotations: StateFlow<Boolean> = _showAnnotations
    /** 默认音色（客户端保存；核心层只在创建时接受默认音色）。 */
    private val _defaultVoice = MutableStateFlow(prefs.getString("defaultVoice", null) ?: DEFAULT_VOICE)
    val defaultVoice: StateFlow<String> = _defaultVoice
    /** 创建核心时使用的默认音色（导入时与之不同则显式指定音色）。 */
    private var coreVoice = _defaultVoice.value
    /** 单句循环：被循环的句子下标（客户端实现）。 */
    private val _loopSentence = MutableStateFlow<Int?>(null)
    val loopSentence: StateFlow<Int?> = _loopSentence
    // —— 翻译与查词 ——
    private val _translator = MutableStateFlow<TranslatorStatus?>(null)
    val translator: StateFlow<TranslatorStatus?> = _translator
    /** 当前文章的译文：句下标 → 译文（智能精翻优先）。 */
    private val _translations = MutableStateFlow<Map<Int, SentenceTranslation>>(emptyMap())
    val translations: StateFlow<Map<Int, SentenceTranslation>> = _translations
    private val _bilingual = MutableStateFlow(prefs.getBoolean("bilingual", false))
    val bilingual: StateFlow<Boolean> = _bilingual
    private val _vocab = MutableStateFlow<List<VocabEntry>>(emptyList())
    val vocab: StateFlow<List<VocabEntry>> = _vocab
    private val _glossary = MutableStateFlow<List<GlossaryTerm>>(emptyList())
    val glossary: StateFlow<List<GlossaryTerm>> = _glossary
    /** 当前文章的举一反三（键 "句:词"）与句子解析（键为句下标）。 */
    private val _insights = MutableStateFlow<Map<String, AiResult<WordInsight>>>(emptyMap())
    val insights: StateFlow<Map<String, AiResult<WordInsight>>> = _insights
    private val _analyses = MutableStateFlow<Map<Int, AiResult<SentenceAnalysis>>>(emptyMap())
    val analyses: StateFlow<Map<Int, AiResult<SentenceAnalysis>>> = _analyses

    /** 搜索用的正文缓存。 */
    private val bodies = HashMap<String, String>()

    private val sink = AudioTrackSink()
    private val listener = object : SpeakerListener {
        // 后台线程回调 → 主线程
        override fun onEvent(event: SpeakerEvent) {
            scope.launch(Dispatchers.Main) { handle(event) }
        }
    }

    fun start() {
        if (core != null) return
        scope.launch {
            val created = withContext(Dispatchers.IO) {
                val modelDir = runCatching { ModelInstaller.install(context) }
                    .onFailure { Log.e(TAG, "model install failed", it) }
                    .getOrNull()
                val dataDir = File(context.filesDir, "data").absolutePath
                runCatching {
                    SpeakerCore(dataDir, modelDir?.absolutePath, coreVoice, sink, listener)
                }.onFailure { Log.e(TAG, "core init failed", it) }.getOrNull()
            } ?: return@launch
            core = created
            withContext(Dispatchers.IO) { seedIfNeeded(created) }
            _voices.value = created.voices()
            _translator.value = runCatching { created.translatorStatus() }.getOrNull()
            _vocab.value = runCatching { created.listVocab() }.getOrNull().orEmpty()
            refreshCache()
            Log.i(TAG, "core ready, voices=${_voices.value.size}")
            refreshArticles()
            _player.value = created.playerState()
            _player.value.articleId?.let { loadTimeline(it) }
            _ready.value = true
        }
    }

    private fun seedIfNeeded(c: SpeakerCore) {
        if (prefs.getBoolean("seeded", false)) return
        if (c.listArticles().isEmpty()) {
            SampleArticles.all.forEach { s ->
                runCatching { c.importText(s.title, s.body, SourceKind.PASTE, null) }
            }
        }
        prefs.edit().putBoolean("seeded", true).apply()
    }

    // —— 事件 ——

    private var refreshing = false
    private var refreshDirty = false

    private fun handle(e: SpeakerEvent) {
        when (e) {
            SpeakerEvent.LibraryChanged -> refreshArticles()
            is SpeakerEvent.SynthesisProgress -> {
                refreshArticles()
                val t = _timeline.value
                if (e.articleId == _player.value.articleId && t != null) {
                    val i = e.sentence.index.toInt()
                    if (i in t.sentences.indices) {
                        val list = t.sentences.toMutableList()
                        list[i] = e.sentence
                        _timeline.value = t.copy(sentences = list)
                    }
                }
            }
            is SpeakerEvent.SynthesisFinished -> {
                refreshArticles()
                if (e.articleId == _player.value.articleId) loadTimeline(e.articleId)
            }
            is SpeakerEvent.SynthesisFailed -> {
                Log.w(TAG, "synthesis failed: ${e.message}")
                refreshArticles()
            }
            is SpeakerEvent.CacheChanged -> _cache.value = e.stats
            is SpeakerEvent.PlayerStateChanged -> onPlayerState(e.state)
            is SpeakerEvent.PlayerPosition -> {
                _position.value = Position(
                    e.positionMs.toLong(), e.bufferedMs.toLong(),
                    e.sentence?.toInt(), e.word?.toInt(), SystemClock.elapsedRealtime(),
                )
                // 单句循环：播到下一句时跳回被循环的句子
                val loop = _loopSentence.value
                val cur = e.sentence?.toInt()
                if (loop != null && cur != null && cur != loop && _player.value.status == PlayerStatus.PLAYING) {
                    runCatching { core?.seekSentence(loop.toUInt()) }
                }
            }
            is SpeakerEvent.TranslationProgress -> if (e.articleId == _player.value.articleId) {
                _translations.value = _translations.value + (e.translation.index.toInt() to e.translation)
            }
            is SpeakerEvent.TranslationsChanged -> if (e.articleId == _player.value.articleId) {
                _analyses.value = emptyMap()
                loadTranslations(e.articleId)
            }
            is SpeakerEvent.WordInsightReady -> if (e.articleId == _player.value.articleId) {
                _insights.value = _insights.value + ("${e.sentence}:${e.word}" to AiResult(value = e.insight, error = e.error))
            }
            is SpeakerEvent.SentenceAnalysisReady -> if (e.articleId == _player.value.articleId) {
                _analyses.value = _analyses.value + (e.sentence.toInt() to AiResult(value = e.analysis, error = e.error))
            }
            is SpeakerEvent.TranslatorChanged -> _translator.value = e.status
            SpeakerEvent.VocabularyChanged -> refreshVocab()
        }
    }

    private fun onPlayerState(s: PlayerState) {
        val prev = _player.value
        _player.value = s
        _position.value = _position.value.copy(
            positionMs = s.positionMs.toLong(), bufferedMs = s.bufferedMs.toLong(),
            atMs = SystemClock.elapsedRealtime(),
        )
        val id = s.articleId
        if (id != null && id != prev.articleId) {
            loadTimeline(id)
            _insights.value = emptyMap()
            _analyses.value = emptyMap()
            _translations.value = emptyMap()
            loadTranslations(id)
        }
        if (id == null) _timeline.value = null
        if (id != prev.articleId) _loopSentence.value = null
        // 单句循环最后一句：播完后回到该句
        val loop = _loopSentence.value
        if (s.status == PlayerStatus.ENDED && prev.status != PlayerStatus.ENDED && loop != null) {
            runCatching { core?.seekSentence(loop.toUInt()); core?.play() }
            return
        }
        // 播完自动播放待播清单的下一篇
        if (s.status == PlayerStatus.ENDED && prev.status != PlayerStatus.ENDED) {
            _upNext.value.firstOrNull()?.let { next ->
                _upNext.value = _upNext.value.drop(1)
                playInternal(next)
            }
        }
    }

    private fun refreshArticles() {
        val c = core ?: return
        if (refreshing) { refreshDirty = true; return }
        refreshing = true
        scope.launch {
            do {
                refreshDirty = false
                val (list, cols) = withContext(Dispatchers.IO) {
                    runCatching { c.listArticles() }.getOrNull() to runCatching { c.listCollections() }.getOrNull()
                }
                if (cols != null) _collections.value = cols
                if (list != null) {
                    _articles.value = list
                    // 已删除的文章移出待播清单
                    val ids = list.map { it.id }.toSet()
                    if (_upNext.value.any { it !in ids }) _upNext.value = _upNext.value.filter { it in ids }
                }
            } while (refreshDirty)
            refreshing = false
        }
    }

    private fun loadTimeline(id: String) {
        val c = core ?: return
        scope.launch {
            val t = withContext(Dispatchers.IO) { runCatching { c.timeline(id) }.getOrNull() }
            if (_player.value.articleId == id || _timeline.value == null) _timeline.value = t
        }
    }

    // —— 翻译与查词 ——

    /** 智能模式实际可用（已打开且模型已下载）。 */
    val smartReady: Boolean get() = _translator.value?.active == TranslationEngine.SMART

    private fun loadTranslations(id: String) {
        val c = core ?: return
        if (!_bilingual.value) return
        scope.launch {
            val list = withContext(Dispatchers.IO) { runCatching { c.translations(id) }.getOrNull() } ?: return@launch
            if (_player.value.articleId == id) _translations.value = list.associateBy { it.index.toInt() }
        }
    }

    /** 打开双语：让核心优先翻译正在播放的文章。 */
    fun setBilingual(on: Boolean) {
        _bilingual.value = on
        prefs.edit().putBoolean("bilingual", on).apply()
        if (on) prepareTranslations()
    }

    private fun prepareTranslations() {
        val id = _player.value.articleId ?: return
        runCatching { core?.prepareTranslations(id) }
        loadTranslations(id)
    }

    fun lookup(word: String): DictEntry? = runCatching { core?.lookupWord(word) }.getOrNull()

    fun requestInsight(sentence: Int, word: Int) {
        val c = core ?: return
        val id = _player.value.articleId ?: return
        val key = "$sentence:$word"
        val cur = _insights.value[key]
        if (cur?.value != null || cur?.loading == true) return
        val r = runCatching { c.wordInsight(id, sentence.toUInt(), word.toUInt()) }
        _insights.value = _insights.value + (key to r.fold(
            { v -> if (v != null) AiResult(value = v) else AiResult(loading = true) },
            { e -> AiResult(error = e.message) },
        ))
    }

    fun requestAnalysis(sentence: Int) {
        val c = core ?: return
        val id = _player.value.articleId ?: return
        val cur = _analyses.value[sentence]
        if (cur?.value != null || cur?.loading == true) return
        val r = runCatching { c.sentenceAnalysis(id, sentence.toUInt()) }
        _analyses.value = _analyses.value + (sentence to r.fold(
            { v -> if (v != null) AiResult(value = v) else AiResult(loading = true) },
            { e -> AiResult(error = e.message) },
        ))
    }

    fun setSmart(on: Boolean) {
        runCatching { core?.setSmartTranslation(on) }
        _translator.value = runCatching { core?.translatorStatus() }.getOrNull()
        if (on) prepareTranslations()
    }

    fun downloadLlm() {
        runCatching { core?.downloadLlm() }.onFailure { Log.w(TAG, "downloadLlm", it) }
        _translator.value = runCatching { core?.translatorStatus() }.getOrNull()
    }

    fun cancelLlmDownload() { core?.cancelLlmDownload() }

    fun deleteLlm() {
        runCatching { core?.setSmartTranslation(false); core?.deleteLlm() }
        _translator.value = runCatching { core?.translatorStatus() }.getOrNull()
    }

    fun refreshGlossary() {
        _glossary.value = runCatching { core?.listGlossary() }.getOrNull() ?: _glossary.value
    }

    /** 失败（如为空）时返回 false。 */
    fun addGlossary(term: String, translation: String): Boolean {
        val ok = runCatching { core?.addGlossaryTerm(term, translation) }.isSuccess
        refreshGlossary()
        return ok
    }

    fun removeGlossary(term: String) {
        runCatching { core?.removeGlossaryTerm(term) }
        refreshGlossary()
    }

    private fun refreshVocab() {
        _vocab.value = runCatching { core?.listVocab() }.getOrNull() ?: _vocab.value
    }

    fun isInVocab(word: String): Boolean = _vocab.value.any { it.word == word.lowercase() }

    fun addVocab(sentence: Int, word: Int) {
        val id = _player.value.articleId ?: return
        runCatching { core?.addVocab(id, sentence.toUInt(), word.toUInt()) }.onFailure { Log.w(TAG, "addVocab", it) }
    }

    fun removeVocab(word: String) { runCatching { core?.removeVocab(word) } }

    // —— 资料库 ——

    fun article(id: String?): ArticleSummary? = id?.let { i -> _articles.value.firstOrNull { it.id == i } }

    fun importText(title: String?, body: String, source: SourceKind, sourceRef: String?): ArticleSummary? {
        val c = core ?: return null
        return runCatching { c.importText(title?.takeIf { it.isNotBlank() }, body, source, sourceRef) }
            .onFailure { Log.e(TAG, "import failed", it) }
            .getOrNull()
            .also { refreshArticles() }
    }

    /** 导入：按所选音色开始转换，可选立即播放（边听边转）。 */
    fun importAndMaybePlay(title: String?, body: String, source: SourceKind, sourceRef: String?, voiceId: String, playNow: Boolean): ArticleSummary? {
        val a = importText(title, body, source, sourceRef) ?: return null
        if (voiceId != coreVoice) synthesize(a.id, voiceId)
        if (playNow) play(a.id)
        return a
    }

    fun delete(id: String) {
        val c = core ?: return
        if (_player.value.articleId == id) c.stop()
        runCatching { c.deleteArticle(id) }
        _upNext.value = _upNext.value - id
        refreshArticles()
    }

    fun synthesize(id: String, voiceId: String? = null) {
        runCatching { core?.synthesize(id, voiceId) }.onFailure { Log.w(TAG, "synthesize", it) }
        refreshArticles()
    }

    fun setPinned(id: String, pinned: Boolean) {
        runCatching { core?.setPinned(id, pinned) }
        refreshArticles()
    }

    fun removeDownload(id: String) {
        runCatching { core?.removeDownload(id) }
        refreshArticles()
        refreshCache()
    }

    // —— 分类 ——

    fun collection(id: String?): ArticleCollection? = id?.let { i -> _collections.value.firstOrNull { it.id == i } }

    /** 分类里的文章，按加入顺序。 */
    fun articlesIn(collectionId: String): List<ArticleSummary> =
        collection(collectionId)?.articleIds.orEmpty().mapNotNull { article(it) }

    /** 文章所在的全部分类。 */
    fun collectionsOf(articleId: String): List<ArticleCollection> =
        _collections.value.filter { articleId in it.articleIds }

    /** 名称校验（与核心一致：去空白后非空、不区分大小写不重名）；合法时返回 null。 */
    fun collectionNameError(name: String, except: String? = null): String? {
        val n = name.trim()
        if (n.isEmpty()) return "请输入分类名称"
        if (_collections.value.any { it.id != except && it.name.equals(n, ignoreCase = true) }) return "已有同名分类"
        return null
    }

    /** 新建分类；给了 articleId 时建好后把这篇加进去。 */
    fun createCollection(name: String, adding: String? = null): ArticleCollection? {
        val c = core ?: return null
        val created = runCatching { c.createCollection(name) }.onFailure { Log.w(TAG, "createCollection", it) }.getOrNull()
        if (created != null && adding != null) runCatching { c.addToCollection(adding, created.id) }
        refreshArticles()
        return created
    }

    fun renameCollection(id: String, name: String) {
        runCatching { core?.renameCollection(id, name) }.onFailure { Log.w(TAG, "renameCollection", it) }
        refreshArticles()
    }

    fun deleteCollection(id: String) {
        runCatching { core?.deleteCollection(id) }
        refreshArticles()
    }

    /** 已在其中则移出，否则加入（菜单里打 ✓ 的项再点一次即移出）。 */
    fun toggleCollection(collectionId: String, articleId: String) {
        val c = core ?: return
        val has = collection(collectionId)?.articleIds?.contains(articleId) ?: return
        runCatching {
            if (has) c.removeFromCollection(articleId, collectionId) else c.addToCollection(articleId, collectionId)
        }.onFailure { Log.w(TAG, "toggleCollection", it) }
        refreshArticles()
    }

    fun addToCollection(collectionId: String, articleId: String) {
        runCatching { core?.addToCollection(articleId, collectionId) }.onFailure { Log.w(TAG, "addToCollection", it) }
        refreshArticles()
    }

    fun removeFromCollection(collectionId: String, articleId: String) {
        runCatching { core?.removeFromCollection(articleId, collectionId) }
        refreshArticles()
    }

    // —— 本地缓存 ——

    fun refreshCache() {
        val c = core ?: return
        scope.launch {
            withContext(Dispatchers.IO) { runCatching { c.cacheStats() }.getOrNull() }?.let { _cache.value = it }
        }
    }

    fun setCacheLimit(bytes: ULong) {
        runCatching { core?.setCacheLimit(bytes) }.onFailure { Log.w(TAG, "setCacheLimit", it) }
        refreshCache()
    }

    /** 清除未钉住的缓存，返回清除的条目数。 */
    fun clearCache(): Int {
        val n = runCatching { core?.clearCache()?.toInt() }.getOrNull() ?: 0
        refreshArticles()
        refreshCache()
        return n
    }

    fun setDefaultVoice(id: String) {
        _defaultVoice.value = id
        prefs.edit().putString("defaultVoice", id).apply()
    }

    /** 搜索：按标题与正文过滤（正文按需取并缓存）。 */
    suspend fun search(query: String): List<Pair<ArticleSummary, String?>> {
        val q = query.trim()
        if (q.isEmpty()) return emptyList()
        val c = core ?: return emptyList()
        val list = _articles.value
        return withContext(Dispatchers.IO) {
            list.mapNotNull { a ->
                val body = synchronized(bodies) { bodies[a.id] }
                    ?: runCatching { c.getArticle(a.id).body }.getOrNull()?.also { synchronized(bodies) { bodies[a.id] = it } }
                    ?: ""
                when {
                    a.title.contains(q, ignoreCase = true) -> a to null
                    body.contains(q, ignoreCase = true) -> a to snippet(body, q)
                    else -> null
                }
            }
        }
    }

    private fun snippet(body: String, q: String): String {
        val i = body.indexOf(q, ignoreCase = true).coerceAtLeast(0)
        val start = (i - 30).coerceAtLeast(0)
        val end = (i + q.length + 60).coerceAtMost(body.length)
        return (if (start > 0) "…" else "") + body.substring(start, end).replace('\n', ' ') +
            (if (end < body.length) "…" else "")
    }

    // —— 播放 ——

    /** 播放一篇文章；`queue` 为其后自动续播的文章（像在专辑里点一首歌）。 */
    fun play(id: String, queue: List<String>? = null) {
        if (queue != null) {
            val i = queue.indexOf(id)
            _upNext.value = if (i >= 0) queue.drop(i + 1) else queue.filter { it != id }
        } else {
            _upNext.value = _upNext.value - id
        }
        playInternal(id)
    }

    private fun playInternal(id: String) {
        val c = core ?: return
        if (_player.value.articleId != id) _timeline.value = null
        runCatching { c.playArticle(id) }.onFailure { Log.e(TAG, "play failed", it) }
        loadTimeline(id)
    }

    fun toggle() {
        val c = core ?: return
        when (_player.value.status) {
            PlayerStatus.IDLE -> article(_player.value.articleId)?.let { playInternal(it.id) }
                ?: _articles.value.firstOrNull()?.let { play(it.id, _articles.value.map { a -> a.id }) }
            else -> c.toggle()
        }
    }

    fun play() = core?.play()
    fun pause() = core?.pause()
    fun seek(ms: Long) = core?.seek(ms.coerceAtLeast(0).toULong())

    fun seekSentence(index: Int) {
        runCatching { core?.seekSentence(index.toUInt()) }
        if (_player.value.status == PlayerStatus.PAUSED || _player.value.status == PlayerStatus.ENDED) core?.play()
    }

    /** 下一句；已是最后一句时播放待播清单的下一篇。 */
    fun nextSentence() {
        val t = _timeline.value ?: return
        val cur = _position.value.sentence ?: -1
        val next = cur + 1
        if (next < t.sentences.size) {
            runCatching { core?.seekSentence(next.toUInt()) }
        } else {
            skipToNextArticle()
        }
    }

    /** 上一句：句内已播超过 2 秒时回到本句开头。 */
    fun previousSentence() {
        val t = _timeline.value ?: return
        val cur = _position.value.sentence ?: 0
        val s = t.sentences.getOrNull(cur)
        val into = if (s != null && s.ready) livePosition() - s.startMs.toLong() else 0L
        val target = if (into > 2000 || cur == 0) cur else cur - 1
        runCatching { core?.seekSentence(target.toUInt()) }
    }

    fun skipToNextArticle() {
        _upNext.value.firstOrNull()?.let { next ->
            _upNext.value = _upNext.value.drop(1)
            playInternal(next)
        }
    }

    fun setRate(rate: Float) = core?.setRate(rate)

    /** 单句循环开关：开启时循环当前句。 */
    fun toggleLoop() {
        _loopSentence.value = if (_loopSentence.value != null) null else (_position.value.sentence ?: 0)
    }

    fun setAnnotations(on: Boolean) {
        _showAnnotations.value = on
        prefs.edit().putBoolean("annotations", on).apply()
    }

    // —— 待播清单 ——

    fun moveUpNext(from: Int, to: Int) {
        val l = _upNext.value.toMutableList()
        if (from !in l.indices || to !in l.indices) return
        l.add(to, l.removeAt(from))
        _upNext.value = l
    }

    fun removeUpNext(id: String) { _upNext.value = _upNext.value - id }
    fun addUpNext(id: String) { if (id !in _upNext.value) _upNext.value = _upNext.value + id }

    /** 两次位置事件之间按倍速插值。 */
    fun livePosition(): Long {
        val p = _position.value
        val s = _player.value
        if (s.status != PlayerStatus.PLAYING) return p.positionMs
        val elapsed = ((SystemClock.elapsedRealtime() - p.atMs) * s.rate).toLong().coerceIn(0, 400)
        return (p.positionMs + elapsed).coerceAtMost(maxOf(p.bufferedMs, p.positionMs))
    }

    fun voice(voiceId: String?): Voice? {
        val vid = voiceId ?: coreVoice
        return _voices.value.firstOrNull { it.id == vid }
    }

    /** 音色标签，例：`Heart · 美音`。 */
    fun voiceLabel(voiceId: String?, sep: String = " · "): String {
        val vid = voiceId ?: coreVoice
        val v = voice(vid) ?: return vid
        return "${v.name}$sep${if (v.accent == Accent.AMERICAN) "美音" else "英音"}"
    }

    /** 副标题（媒体通知用）：音色，例：`Heart · 美音`。 */
    fun subtitle(a: ArticleSummary): String = voiceLabel(a.voiceId)

    companion object {
        private const val TAG = "SpeakerRepo"
        const val DEFAULT_VOICE = "af_heart"
    }
}
