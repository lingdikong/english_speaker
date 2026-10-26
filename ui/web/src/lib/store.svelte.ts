// 应用状态：业务数据全部来自核心层（invoke + 事件）；UI 只维护展示偏好与待播清单（客户端队列）。

import { invoke, onEvent } from './bridge';
import type {
  Article,
  ArticleSummary,
  CacheStats,
  Collection,
  CoreEvent,
  DictEntry,
  GlossaryTerm,
  HostInfo,
  PlayerState,
  SentenceAnalysis,
  SentenceSpan,
  SentenceTranslation,
  Timeline,
  TranslatorStatus,
  VocabEntry,
  Voice,
  WordInsight,
  WordSpan,
} from './types';

export type ListFilter = 'all' | 'converting' | 'saved' | 'unconverted';
export type View =
  | { kind: 'home' }
  | { kind: 'list'; filter: ListFilter }
  | { kind: 'collection'; id: string }
  | { kind: 'search' }
  | { kind: 'vocab' }
  | { kind: 'settings' };

export type Theme = 'system' | 'light' | 'dark';

export interface Prefs {
  theme: Theme;
  /** 阅读字号（移动端当前句标准 24，范围 20–30；PC 面板当前句 = 字号 − 1）。 */
  fontSize: number;
  lineHeight: 1.6 | 1.4;
  /** 非当前句亮度：50 / 60 / 70（%）。 */
  otherLevel: 50 | 60 | 70;
  showNotes: boolean;
  /** 双语：每句下方显示中文译文。 */
  bilingual: boolean;
  defaultVoice: string | null;
  defaultRate: number;
  volume: number;
}

const DEFAULT_PREFS: Prefs = {
  theme: 'system',
  fontSize: 24,
  lineHeight: 1.6,
  otherLevel: 60,
  showNotes: true,
  bilingual: false,
  defaultVoice: null,
  defaultRate: 1,
  volume: 1,
};

export const RATES = [0.75, 1, 1.25, 1.5];

/** 拖动文章行时 dataTransfer 的类型（拖到分类卡片上即移入）。 */
export const ARTICLE_MIME = 'application/x-es-article';

export interface MenuItem {
  label?: string;
  action?: () => void;
  disabled?: boolean;
  checked?: boolean;
  danger?: boolean;
  separator?: boolean;
  submenu?: MenuItem[];
}

export interface WordPopover {
  sentence: SentenceSpan;
  wordIndex: number;
  rect: DOMRect;
  /** 当前句卡片：popover 放在卡片上下方，不遮住句子。 */
  box?: DOMRect;
}

export interface Confirm {
  title: string;
  body: string;
  ok: string;
  danger?: boolean;
  /** 带输入框（新建 / 重命名分类）：初始值与占位文字；确定时把输入内容交给 action。 */
  input?: { value: string; placeholder: string; validate?: (value: string) => string | null };
  action: (value: string) => void | Promise<unknown>;
}

/** 智能解析（点词举一反三 / 句子解析）的请求状态，按 文章:句[:词] 索引。 */
export interface AiResult<T> {
  loading: boolean;
  value: T | null;
  error: string | null;
}

const IDLE: PlayerState = {
  article_id: null,
  status: 'idle',
  position_ms: 0,
  buffered_ms: 0,
  duration_ms: 0,
  duration_final: false,
  rate: 1,
};

function loadPrefs(): Prefs {
  try {
    const v = localStorage.getItem('es.prefs');
    return { ...DEFAULT_PREFS, ...(v ? (JSON.parse(v) as Partial<Prefs>) : {}) };
  } catch {
    return { ...DEFAULT_PREFS };
  }
}

function loadJson<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(`es.${key}`);
    return v == null ? fallback : (JSON.parse(v) as T);
  } catch {
    return fallback;
  }
}

function saveJson(key: string, v: unknown) {
  try {
    localStorage.setItem(`es.${key}`, JSON.stringify(v));
  } catch {
    /* 隐私模式等情况下忽略 */
  }
}

const darkQuery = window.matchMedia('(prefers-color-scheme: dark)');

class Store {
  articles = $state<ArticleSummary[]>([]);
  collections = $state<Collection[]>([]);
  loaded = $state(false);
  voices = $state<Voice[]>([]);
  host = $state<HostInfo>({ host: 'unknown', volume: 1, rate_supported: false, engine: '' });
  cache = $state<CacheStats | null>(null);

  player = $state<PlayerState>({ ...IDLE });
  /** 最近一次 PlayerPosition 事件 + 收到时刻（用于插值，让逐词高亮平滑）。 */
  pos = $state({ position_ms: 0, buffered_ms: 0, sentence: null as number | null, at: 0 });
  /** rAF 驱动的时钟，仅播放时更新。 */
  now = $state(0);
  /** 拖动声波进度时的目标位置。 */
  scrubbing = $state<number | null>(null);

  timelines = $state<Record<string, Timeline>>({});
  /** 每篇的译文：句下标 → 译文（智能精翻优先）。 */
  translations = $state<Record<string, Record<number, SentenceTranslation>>>({});
  translator = $state<TranslatorStatus | null>(null);
  glossary = $state<GlossaryTerm[]>([]);
  vocab = $state<VocabEntry[]>([]);
  insights = $state<Record<string, AiResult<WordInsight>>>({});
  analyses = $state<Record<string, AiResult<SentenceAnalysis>>>({});
  /** get_article 的正文与预计时长（搜索、未转换文章的“约”时长）。 */
  details = $state<Record<string, { body: string; estimated_ms: number }>>({});

  view = $state<View>({ kind: 'home' });
  search = $state('');
  prefs = $state<Prefs>(loadPrefs());
  systemDark = $state(darkQuery.matches);

  importOpen = $state(false);
  /** 拖入窗口的文件：打开导入对话框时预填。 */
  importFile = $state<{ name: string; text: string } | null>(null);
  queueOpen = $state(false);
  /** 全屏播放页（点播放条封面打开）。 */
  playerOpen = $state(false);
  /** 播放页开合动画的小球：播放条封面的中心与半径（开 / 合前量好，避免动画开始时强制布局）。 */
  playerOrigin = { x: 0, y: 0, r: 19 };
  readingOpen = $state(false);
  /** 单句循环（客户端：离开该句时跳回句首）。 */
  loop = $state(false);
  loopIndex: number | null = null;

  /** 待播清单（客户端维护）。 */
  queue = $state<string[]>(loadJson('queue', []));

  menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  popover = $state<WordPopover | null>(null);
  confirm = $state<Confirm | null>(null);
  toast = $state<string | null>(null);

  // ---------- 派生 ----------

  current = $derived(this.articles.find((a) => a.id === this.player.article_id) ?? null);
  timeline = $derived(this.player.article_id ? (this.timelines[this.player.article_id] ?? null) : null);
  /** 播放或缓冲中（播放按钮显示“暂停”）。 */
  active = $derived(this.player.status === 'playing' || this.player.status === 'buffering');
  dark = $derived(this.prefs.theme === 'system' ? this.systemDark : this.prefs.theme === 'dark');
  /** 有整句翻译能力（Fast 或 Smart 任一可用）。 */
  canTranslate = $derived(this.translator?.active != null);
  /** 智能模式实际可用（已打开且模型已下载）。 */
  smart = $derived(this.translator?.active === 'smart');
  /** 生词本里的词（原形小写），点词面板据此显示“已收藏”。 */
  vocabWords = $derived(new Set(this.vocab.map((v) => v.word)));

  /** 插值后的播放位置。 */
  position = $derived.by(() => {
    if (this.scrubbing != null) return this.scrubbing;
    const p = this.pos;
    if (this.player.status !== 'playing') return p.position_ms;
    const t = p.position_ms + Math.max(0, this.now - p.at) * this.player.rate;
    // 前探不超过一个事件间隔：否则每次 player_position 到达都会明显回跳，
    // 逐词高亮（单词框）来回翻转看起来就是闪烁。
    const cap = Math.min(400, Math.max(100, this.evInterval * 1.6));
    return Math.min(t, Math.max(p.buffered_ms, p.position_ms), p.position_ms + cap);
  });
  buffered = $derived(Math.max(this.pos.buffered_ms, this.player.buffered_ms));
  duration = $derived(Math.max(this.player.duration_ms, this.current?.duration_ms ?? 0));

  /** 当前句与当前词（按插值位置在时间轴上定位）。 */
  located = $derived.by(() => locate(this.timeline, this.position, this.pos.sentence));

  counts = $derived.by(() => {
    const c = { all: 0, converting: 0, saved: 0, unconverted: 0 };
    for (const a of this.articles) {
      c.all++;
      if (isConverting(a)) c.converting++;
      if (a.pinned) c.saved++;
      if (a.status === 'not_cached') c.unconverted++;
    }
    return c;
  });

  constructor() {
    const tick = (t: number) => {
      if (this.player.status === 'playing') this.now = t;
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
    darkQuery.addEventListener('change', (e) => (this.systemDark = e.matches));
  }

  // ---------- 初始化与事件 ----------

  async init() {
    onEvent((e) => this.handle(e));
    const [articles, collections, voices, player, cache, host, translator, vocab] = await Promise.all([
      invoke<ArticleSummary[]>('list_articles'),
      invoke<Collection[]>('list_collections'),
      invoke<Voice[]>('voices'),
      invoke<PlayerState>('player_state'),
      invoke<CacheStats>('cache_stats'),
      invoke<HostInfo>('host_info').catch(() => null),
      invoke<TranslatorStatus>('translator_status').catch(() => null),
      invoke<VocabEntry[]>('list_vocab').catch(() => []),
    ]);
    this.translator = translator;
    this.vocab = vocab;
    this.articles = articles;
    this.collections = collections;
    this.voices = voices;
    this.cache = cache;
    if (host) this.host = host;
    this.applyPlayer(player);
    this.pos = { position_ms: player.position_ms, buffered_ms: player.buffered_ms, sentence: null, at: performance.now() };
    this.queue = this.queue.filter((id) => articles.some((a) => a.id === id));
    // 客户端偏好 → 宿主：音量、默认倍速
    if (Math.abs(this.prefs.volume - (host?.volume ?? 1)) > 0.001) this.setVolume(this.prefs.volume);
    if (this.host.rate_supported && player.status === 'idle' && player.rate !== this.prefs.defaultRate) {
      invoke('set_rate', { rate: this.prefs.defaultRate }).catch(() => {});
    }
    this.loaded = true;
    this.loadDetails();
  }

  private refreshTimer = 0;
  refreshLibrary() {
    clearTimeout(this.refreshTimer);
    this.refreshTimer = window.setTimeout(async () => {
      [this.articles, this.collections] = await Promise.all([
        invoke<ArticleSummary[]>('list_articles'),
        invoke<Collection[]>('list_collections'),
      ]);
      // 正在看的分类被删除：回到全部文章
      const v = this.view;
      if (v.kind === 'collection' && !this.collections.some((c) => c.id === v.id)) this.view = { kind: 'list', filter: 'all' };
      const ids = new Set(this.articles.map((a) => a.id));
      const q = this.queue.filter((id) => ids.has(id));
      if (q.length !== this.queue.length) this.setQueue(q);
      this.loadDetails();
    }, 60);
  }

  async refreshCache() {
    this.cache = await invoke<CacheStats>('cache_stats');
  }

  /** 搜索与“约”时长需要正文 / 预计时长：每篇取一次。 */
  private async loadDetails() {
    for (const a of this.articles) {
      if (this.details[a.id] !== undefined) continue;
      try {
        const full = await invoke<Article>('get_article', { id: a.id });
        this.details[a.id] = { body: full.body, estimated_ms: full.estimated_ms };
      } catch {
        /* 文章可能刚被删除 */
      }
    }
  }

  private handle(e: CoreEvent) {
    switch (e.type) {
      case 'library_changed':
        this.refreshLibrary();
        break;
      case 'synthesis_progress': {
        const a = this.articles.find((x) => x.id === e.article_id);
        if (a) {
          a.ready_sentences = e.ready;
          a.total_sentences = e.total;
          if (a.status !== 'synthesizing') a.status = 'synthesizing';
        }
        const t = this.timelines[e.article_id];
        if (t && t.sentences[e.sentence.index]) t.sentences[e.sentence.index] = e.sentence;
        else if (!t && e.article_id === this.player.article_id) this.loadTimeline(e.article_id);
        break;
      }
      case 'synthesis_finished':
        this.refreshLibrary();
        this.loadTimeline(e.article_id);
        break;
      case 'synthesis_failed':
        this.refreshLibrary();
        this.flash(`转换失败：${e.message}`);
        break;
      case 'cache_changed':
        this.cache = e.stats;
        break;
      case 'player_state_changed':
        this.applyPlayer(e.state);
        break;
      case 'translation_progress': {
        const m = this.translations[e.article_id];
        if (m) m[e.translation.index] = e.translation;
        break;
      }
      case 'translations_changed':
        if (this.translations[e.article_id]) this.loadTranslations(e.article_id);
        for (const k of Object.keys(this.analyses)) if (k.startsWith(`${e.article_id}:`)) delete this.analyses[k];
        break;
      case 'word_insight_ready':
        this.insights[`${e.article_id}:${e.sentence}:${e.word}`] = { loading: false, value: e.insight, error: e.error };
        break;
      case 'sentence_analysis_ready':
        this.analyses[`${e.article_id}:${e.sentence}`] = { loading: false, value: e.analysis, error: e.error };
        break;
      case 'translator_changed':
        this.translator = e.status;
        break;
      case 'vocabulary_changed':
        this.loadVocab();
        break;
      case 'player_position': {
        // 记录事件实际到达间隔（EMA），供位置插值限制前探幅度，避免逐词高亮回跳
        const at = performance.now();
        const delta = at - this.pos.at;
        if (delta > 20 && delta < 1000) this.evInterval += (delta - this.evInterval) * 0.3;
        this.pos = {
          position_ms: e.position_ms,
          buffered_ms: e.buffered_ms,
          sentence: e.sentence,
          at,
        };
        this.now = at;
        this.watchPosition();
        break;
      }
    }
  }

  private applyPlayer(state: PlayerState) {
    const prev = this.player;
    const prevPos = this.pos.position_ms;
    this.player = state;
    if (state.status !== 'playing') {
      this.pos = { ...this.pos, position_ms: state.position_ms, buffered_ms: state.buffered_ms, at: performance.now() };
    }
    if (state.article_id && state.article_id !== prev.article_id) {
      // 换篇时核心会写入上一篇的进度，但已转换完的文章不会触发 library_changed：
      // 先按最后位置更新本地进度，再刷新资料库，否则“继续收听”里上一篇的进度停在旧值（常为 0 而消失）。
      const old = prev.article_id ? this.article(prev.article_id) : null;
      if (old) old.progress_ms = prev.status === 'ended' ? 0 : prevPos;
      this.refreshLibrary();
      this.loopIndex = null;
      this.loadTimeline(state.article_id);
      if (this.prefs.bilingual) this.loadTranslations(state.article_id);
    }
    if (state.status === 'ended' && prev.status !== 'ended') {
      // 单句循环在最后一句：回到句首；否则自动播待播清单中的下一篇
      if (this.loop && this.loopIndex != null) this.seekSentence(this.loopIndex);
      else this.advance();
    }
  }

  /** player_position 事件的实际到达间隔（EMA，毫秒）：核心约每 100ms 推一次。 */
  private evInterval = 100;
  /** 位置事件到达时：单句循环、单词试听、慢放结束后恢复倍速。 */
  private watchPosition() {
    const ms = this.pos.position_ms;
    const sentence = this.pos.sentence;
    if (this.wordStop != null && ms >= this.wordStop) {
      this.wordStop = null;
      invoke('pause');
    }
    if (this.slow && sentence != null && sentence !== this.slow.index) {
      invoke('set_rate', { rate: this.slow.rate }).catch(() => {});
      this.slow = null;
    }
    if (this.loop && this.player.status === 'playing') {
      if (this.loopIndex == null) this.loopIndex = sentence;
      else if (sentence != null && sentence !== this.loopIndex) this.seekSentence(this.loopIndex);
    }
  }

  async loadTimeline(id: string) {
    try {
      this.timelines[id] = await invoke<Timeline>('timeline', { id });
    } catch {
      /* 忽略 */
    }
  }

  // ---------- 翻译与查词 ----------

  async loadTranslations(id: string) {
    try {
      const list = await invoke<SentenceTranslation[]>('translations', { id });
      const m: Record<number, SentenceTranslation> = {};
      for (const t of list) m[t.index] = t;
      this.translations[id] = m;
    } catch {
      /* 文章可能刚被删除 */
    }
  }

  /** 打开 / 关闭双语；打开时让核心优先翻译正在播放的文章。 */
  toggleBilingual() {
    this.setPref('bilingual', !this.prefs.bilingual);
    const id = this.player.article_id;
    if (this.prefs.bilingual && id) {
      invoke('prepare_translations', { id }).catch(() => {});
      this.loadTranslations(id);
    }
  }

  /** 打开智能翻译时若模型未下载，先确认下载（约 430MB）。 */
  setSmart(on: boolean) {
    const llm = this.translator?.llm;
    if (on && llm && llm.state !== 'downloaded' && llm.state !== 'downloading') {
      this.confirm = {
        title: '下载智能翻译模型？',
        body: `需要下载 ${llm.model_name}（约 ${fmtBytes(llm.total_bytes)}），下载后完全离线运行，会占用约 500 MB 内存。可随时在设置中删除。`,
        ok: '下载并开启',
        action: async () => {
          await invoke('set_smart_translation', { enabled: true });
          await invoke('download_llm');
        },
      };
      return;
    }
    this.guard(async () => {
      this.translator = await invoke<TranslatorStatus>('set_smart_translation', { enabled: on });
      const id = this.player.article_id;
      if (on && id) invoke('prepare_translations', { id }).catch(() => {});
    });
  }

  downloadLlm() {
    this.guard(() => invoke('download_llm'));
  }

  cancelLlmDownload() {
    invoke('cancel_llm_download').catch(() => {});
  }

  deleteLlm() {
    this.confirm = {
      title: '删除智能翻译模型？',
      body: '删除后智能翻译、举一反三和句子解析不可用，整句翻译退回极速模式。已生成的译文与解析会保留。',
      ok: '删除',
      danger: true,
      action: async () => {
        await invoke('set_smart_translation', { enabled: false });
        await invoke('delete_llm');
      },
    };
  }

  async loadGlossary() {
    this.glossary = await invoke<GlossaryTerm[]>('list_glossary').catch(() => []);
  }

  async addGlossary(term: string, translation: string): Promise<boolean> {
    const ok = await this.guard(() => invoke('add_glossary_term', { term, translation }));
    if (ok) await this.loadGlossary();
    return ok;
  }

  async removeGlossary(term: string) {
    await this.guard(() => invoke('remove_glossary_term', { term }));
    await this.loadGlossary();
  }

  async loadVocab() {
    this.vocab = await invoke<VocabEntry[]>('list_vocab').catch(() => this.vocab);
  }

  async addVocab(id: string, sentence: number, word: number) {
    try {
      const v = await invoke<VocabEntry>('add_vocab', { id, sentence, word });
      this.flash(`已加入生词本：${v.word}`);
    } catch (e) {
      this.flash(String((e as Error).message ?? e));
    }
  }

  async removeVocab(word: string) {
    await this.guard(() => invoke('remove_vocab', { word }));
  }

  lookup(word: string): Promise<DictEntry | null> {
    return invoke<DictEntry | null>('lookup_word', { word }).catch(() => null);
  }

  /** 点词“举一反三”：有缓存立即可见，否则后台生成，结果经事件写入 `insights`。 */
  async requestInsight(id: string, sentence: number, word: number) {
    const key = `${id}:${sentence}:${word}`;
    if (this.insights[key]?.value || this.insights[key]?.loading) return;
    this.insights[key] = { loading: true, value: null, error: null };
    try {
      const v = await invoke<WordInsight | null>('word_insight', { id, sentence, word });
      if (v) this.insights[key] = { loading: false, value: v, error: null };
    } catch (e) {
      this.insights[key] = { loading: false, value: null, error: String((e as Error).message ?? e) };
    }
  }

  async requestAnalysis(id: string, sentence: number) {
    const key = `${id}:${sentence}`;
    if (this.analyses[key]?.value || this.analyses[key]?.loading) return;
    this.analyses[key] = { loading: true, value: null, error: null };
    try {
      const v = await invoke<SentenceAnalysis | null>('sentence_analysis', { id, sentence });
      if (v) this.analyses[key] = { loading: false, value: v, error: null };
    } catch (e) {
      this.analyses[key] = { loading: false, value: null, error: String((e as Error).message ?? e) };
    }
  }

  flash(msg: string) {
    this.toast = msg;
    setTimeout(() => {
      if (this.toast === msg) this.toast = null;
    }, 3200);
  }

  // ---------- 展示辅助 ----------

  article(id: string) {
    return this.articles.find((a) => a.id === id) ?? null;
  }

  voiceOf(a: ArticleSummary | null): Voice | undefined {
    if (!a?.voice_id) return undefined;
    return this.voices.find((v) => v.id === a.voice_id);
  }

  /** “Heart · 美音”；尚未指定音色的文章返回 “—”。 */
  voiceText(a: ArticleSummary | null): string {
    const v = this.voiceOf(a);
    return v ? voiceLabel(v) : '—';
  }

  /** 时长：已完成 → 精确；否则 “约 m:00”（预计）。 */
  durationText(a: ArticleSummary): string {
    if (a.status === 'ready' && a.duration_ms > 0) return fmtTime(a.duration_ms);
    const est = Math.max(a.duration_ms, this.details[a.id]?.estimated_ms ?? 0);
    if (!est) return '—';
    return `约 ${Math.max(1, Math.round(est / 60000))}:00`;
  }

  // ---------- 偏好 ----------

  setPref<K extends keyof Prefs>(k: K, v: Prefs[K]) {
    this.prefs[k] = v;
    saveJson('prefs', $state.snapshot(this.prefs));
  }

  toggleNotes() {
    this.setPref('showNotes', !this.prefs.showNotes);
  }

  openPlayer() {
    if (!this.player.article_id) return;
    this.measureOrigin();
    this.queueOpen = false;
    this.playerOpen = true;
  }

  closePlayer() {
    this.measureOrigin();
    this.queueOpen = false;
    this.popover = null;
    this.playerOpen = false;
  }

  /** 小球位置：播放条封面中心；找不到时退到窗口底部中央。 */
  private measureOrigin() {
    const r = document.querySelector('[data-player-origin]')?.getBoundingClientRect();
    this.playerOrigin =
      r && r.width
        ? { x: r.left + r.width / 2, y: r.top + r.height / 2, r: r.width / 2 }
        : { x: window.innerWidth / 2, y: window.innerHeight - 46, r: 19 };
  }

  // ---------- 播放控制 ----------

  async play(id: string, sentence?: number) {
    try {
      if (this.player.article_id !== id || this.player.status === 'ended' || this.player.status === 'idle') {
        this.setQueue(this.queue.filter((x) => x !== id));
        await invoke('play_article', { id });
      } else if (!this.active) {
        await invoke('play');
      }
      if (sentence != null) await invoke('seek_sentence', { index: sentence });
    } catch (err) {
      this.flash(String((err as Error).message ?? err));
    }
  }

  toggle() {
    if (!this.player.article_id) {
      const first = this.queue[0] ?? this.articles[0]?.id;
      if (first) this.play(first);
      return;
    }
    if (this.player.status === 'ended') {
      this.play(this.player.article_id);
      return;
    }
    this.wordStop = null;
    invoke('toggle');
  }

  seek(ms: number) {
    const target = Math.max(0, Math.min(ms, this.buffered));
    this.pos = { ...this.pos, position_ms: target, at: performance.now() };
    if (this.loop) this.loopIndex = null;
    invoke('seek', { position_ms: Math.round(target) });
  }

  skip(delta: number) {
    if (!this.player.article_id) return;
    if (this.loop) this.loopIndex = null;
    invoke('skip', { delta_ms: delta });
  }

  seekSentence(index: number) {
    const s = this.timeline?.sentences[index];
    if (!s?.ready) return;
    this.pos = { ...this.pos, position_ms: s.start_ms, sentence: index, at: performance.now() };
    if (this.loop) this.loopIndex = index;
    if (this.player.status === 'ended' || this.player.status === 'idle') {
      this.play(this.player.article_id!, index);
      return;
    }
    invoke('seek_sentence', { index }).catch((e) => this.flash(String(e.message)));
  }

  /** 上一句：句内已播超过 1.5 秒时回到本句开头，否则到上一句。 */
  prevSentence() {
    const t = this.timeline;
    const cur = this.located.sentence;
    if (!t || cur == null) return;
    const s = t.sentences[cur];
    const target = this.position - s.start_ms > 1500 || cur === 0 ? cur : cur - 1;
    this.seekSentence(target);
  }

  nextSentence() {
    const t = this.timeline;
    const cur = this.located.sentence ?? -1;
    if (!t) return;
    if (cur + 1 < t.sentences.length && t.sentences[cur + 1].ready) this.seekSentence(cur + 1);
    else if (cur + 1 >= t.sentences.length) this.advance();
  }

  toggleLoop() {
    this.loop = !this.loop;
    this.loopIndex = this.loop ? this.located.sentence : null;
  }

  cycleRate() {
    if (!this.host.rate_supported) {
      this.flash('当前音频输出不支持变速');
      return;
    }
    const i = RATES.findIndex((r) => Math.abs(r - this.player.rate) < 0.01);
    const rate = RATES[(i + 1) % RATES.length];
    this.slow = null;
    invoke('set_rate', { rate }).catch((e) => this.flash(String(e.message)));
  }

  setVolume(v: number) {
    this.host.volume = v;
    this.setPref('volume', v);
    invoke('set_volume', { volume: v }).catch(() => {});
  }

  /** 单词试听：从词首播到词尾后自动暂停。 */
  private wordStop: number | null = null;
  async playSpan(start_ms: number, end_ms: number) {
    this.wordStop = end_ms;
    this.seek(start_ms);
    if (!this.active) await invoke('play');
  }

  /** 0.75× 慢放这句：离开这句后恢复原倍速。 */
  private slow: { index: number; rate: number } | null = null;
  async slowSentence(index: number) {
    if (!this.host.rate_supported) {
      this.flash('当前音频输出不支持变速');
      return;
    }
    const rate = this.slow?.rate ?? this.player.rate;
    await invoke('set_rate', { rate: 0.75 });
    this.slow = { index, rate };
    this.seekSentence(index);
    if (!this.active) await invoke('play');
  }

  // ---------- 待播清单 ----------

  setQueue(q: string[]) {
    this.queue = q;
    saveJson('queue', q);
  }

  playNext(id: string) {
    this.setQueue([id, ...this.queue.filter((x) => x !== id)]);
  }

  playLater(id: string) {
    this.setQueue([...this.queue.filter((x) => x !== id), id]);
  }

  removeFromQueue(i: number) {
    this.setQueue(this.queue.filter((_, j) => j !== i));
  }

  moveInQueue(from: number, to: number) {
    const q = [...this.queue];
    const [x] = q.splice(from, 1);
    q.splice(to, 0, x);
    this.setQueue(q);
  }

  advance() {
    const next = this.queue[0];
    if (!next) return;
    this.setQueue(this.queue.slice(1));
    this.play(next);
  }

  // ---------- 分类 ----------

  collection(id: string) {
    return this.collections.find((c) => c.id === id) ?? null;
  }

  /** 分类里的文章，按加入顺序（同歌单）。 */
  inCollection(id: string) {
    const ids = this.collection(id)?.article_ids ?? [];
    return ids.map((x) => this.article(x)).filter((a) => a != null);
  }

  /** 文章所在的全部分类。 */
  collectionsOf(articleId: string) {
    return this.collections.filter((c) => c.article_ids.includes(articleId));
  }

  /** 分类名校验（与核心一致：去空白后非空、不区分大小写不重名）。 */
  private nameError(name: string, except?: string): string | null {
    const n = name.trim().toLowerCase();
    if (!n) return '请输入分类名称';
    if (this.collections.some((c) => c.id !== except && c.name.toLowerCase() === n)) return '已有同名分类';
    return null;
  }

  /** 弹出命名对话框新建分类；给了 articleId 时建好后把这篇加进去。 */
  newCollection(articleId?: string) {
    this.confirm = {
      title: '新建分类',
      body: articleId ? '建好后会把这篇文章加进去。' : '把文章整理到分类里，像歌单一样一起听。一篇文章可以放进多个分类。',
      ok: '创建',
      input: { value: '', placeholder: '分类名称，如“新闻”“睡前故事”', validate: (v) => this.nameError(v) },
      action: async (name) => {
        const c = await invoke<Collection>('create_collection', { name });
        if (articleId) await invoke('add_to_collection', { id: articleId, collection_id: c.id });
        this.refreshLibrary();
        if (articleId) this.flash(`已添加到“${c.name}”`);
      },
    };
  }

  renameCollection(c: Collection) {
    this.confirm = {
      title: '重命名分类',
      body: '',
      ok: '保存',
      input: { value: c.name, placeholder: '分类名称', validate: (v) => this.nameError(v, c.id) },
      action: async (name) => {
        await invoke('rename_collection', { id: c.id, name });
        this.refreshLibrary();
      },
    };
  }

  deleteCollection(c: Collection) {
    const n = this.inCollection(c.id).length;
    this.confirm = {
      title: '删除分类？',
      body: n ? `“${c.name}”里的 ${n} 篇文章不会被删除，只是不再属于这个分类。` : `删除空分类“${c.name}”。`,
      ok: '删除',
      danger: true,
      action: async () => {
        await invoke('delete_collection', { id: c.id });
        this.refreshLibrary();
      },
    };
  }

  async addToCollection(articleId: string, collectionId: string) {
    const c = this.collection(collectionId);
    if (!c) return;
    if (c.article_ids.includes(articleId)) {
      this.flash(`已在“${c.name}”中`);
      return;
    }
    if (!(await this.guard(() => invoke('add_to_collection', { id: articleId, collection_id: collectionId })))) return;
    c.article_ids.push(articleId);
    this.flash(`已添加到“${c.name}”`);
    this.refreshLibrary();
  }

  async removeFromCollection(articleId: string, collectionId: string) {
    const c = this.collection(collectionId);
    if (!c) return;
    if (!(await this.guard(() => invoke('remove_from_collection', { id: articleId, collection_id: collectionId })))) return;
    c.article_ids = c.article_ids.filter((x) => x !== articleId);
    this.flash(`已从“${c.name}”移除`);
    this.refreshLibrary();
  }

  /** 播放分类：从第一篇开始，其余依次排进待播清单。 */
  playCollection(id: string) {
    const [first, ...rest] = this.inCollection(id).map((a) => a.id);
    if (!first) return;
    this.setQueue(rest);
    this.play(first);
  }

  collectionMenu(c: Collection): MenuItem[] {
    return [
      { label: '播放全部', action: () => this.playCollection(c.id), disabled: !this.inCollection(c.id).length },
      { label: '重命名', action: () => this.renameCollection(c) },
      { separator: true },
      { label: '删除分类', danger: true, action: () => this.deleteCollection(c) },
    ];
  }

  // ---------- 资料库操作 ----------

  async importText(body: string, opts: { title?: string; sourceRef?: string; voice?: string | null; play?: boolean }) {
    const a = await invoke<ArticleSummary>('import_text', {
      title: opts.title || null,
      body,
      source_ref: opts.sourceRef ?? null,
    });
    this.articles = [a, ...this.articles.filter((x) => x.id !== a.id)];
    // 核心的 import_text 不带音色：导入后立即指定（换音色会丢弃刚开始的默认音色缓存）
    if (opts.voice) await invoke('synthesize', { id: a.id, voice_id: opts.voice }).catch(() => {});
    if (opts.play) await this.play(a.id);
    return a;
  }

  async setPinned(id: string, pinned: boolean) {
    await this.guard(() => invoke('set_pinned', { id, pinned }));
    this.refreshLibrary();
  }

  async synthesize(id: string, voice?: string) {
    await this.guard(() => invoke('synthesize', { id, voice_id: voice ?? null }));
    this.refreshLibrary();
    if (voice) this.loadTimeline(id);
  }

  async cancelSynthesis(id: string) {
    await this.guard(() => invoke('cancel_synthesis', { id }));
    this.refreshLibrary();
  }

  async removeDownload(id: string) {
    await this.guard(() => invoke('remove_download', { id }));
    this.refreshLibrary();
  }

  async remove(id: string) {
    if (!(await this.guard(() => invoke('delete_article', { id })))) return;
    this.setQueue(this.queue.filter((x) => x !== id));
    this.refreshLibrary();
  }

  async setCacheLimit(bytes: number) {
    await this.guard(() => invoke('set_cache_limit', { bytes }));
    this.refreshCache();
  }

  async clearCache() {
    try {
      const n = await invoke<number>('clear_cache');
      this.flash(n ? `已清除 ${n} 篇文章的缓存` : '没有可清除的缓存');
    } catch (e) {
      this.flash(String((e as Error).message ?? e));
    }
    this.refreshCache();
    this.refreshLibrary();
  }

  /** 执行桥接调用，失败时以提示条显示核心返回的错误。 */
  private async guard(fn: () => Promise<unknown>): Promise<boolean> {
    try {
      await fn();
      return true;
    } catch (e) {
      this.flash(String((e as Error).message ?? e));
      return false;
    }
  }

  /** 文章的更多操作（⋯ / 右键）；在分类页里（inCollection）多一项“从此分类移除”。 */
  articleMenu(a: ArticleSummary, inCollection?: string): MenuItem[] {
    const converting = isConverting(a);
    return [
      { label: '播放', action: () => this.play(a.id) },
      { label: '下一篇播放', action: () => this.playNext(a.id) },
      { label: '加入待播清单', action: () => this.playLater(a.id) },
      { separator: true },
      a.pinned
        ? { label: '取消保存', action: () => this.setPinned(a.id, false) }
        : { label: '保存（不被自动清理）', action: () => this.setPinned(a.id, true) },
      converting
        ? { label: '暂停转换', action: () => this.cancelSynthesis(a.id) }
        : a.status === 'ready' || a.status === 'partial'
          ? { label: '删除缓存', action: () => this.removeDownload(a.id), disabled: a.cached_bytes === 0 }
          : { label: a.status === 'failed' ? '重试转换' : '开始转换', action: () => this.synthesize(a.id) },
      {
        // 已在其中的打 ✓，再点一次即移出
        label: '添加到分类',
        submenu: [
          ...this.collections.map((c) => {
            const has = c.article_ids.includes(a.id);
            return {
              label: c.name,
              checked: has,
              action: () => (has ? this.removeFromCollection(a.id, c.id) : this.addToCollection(a.id, c.id)),
            };
          }),
          { label: '新建分类…', action: () => this.newCollection(a.id) },
        ],
      },
      ...(inCollection ? [{ label: '从此分类移除', action: () => this.removeFromCollection(a.id, inCollection) }] : []),
      {
        label: '换音色',
        submenu: this.voices.map((v) => ({
          label: voiceLabel(v),
          checked: a.voice_id === v.id,
          action: () => this.synthesize(a.id, v.id),
        })),
      },
      { separator: true },
      {
        label: '删除文章',
        danger: true,
        action: () =>
          (this.confirm = {
            title: '删除文章？',
            body: `“${a.title}”及其音频缓存将从本机删除。`,
            ok: '删除',
            danger: true,
            action: () => this.remove(a.id),
          }),
      },
    ];
  }

  openMenu(e: MouseEvent, items: MenuItem[]) {
    e.preventDefault();
    e.stopPropagation();
    this.menu = { x: e.clientX, y: e.clientY, items };
  }
}

export function isConverting(a: ArticleSummary): boolean {
  return a.status === 'queued' || a.status === 'synthesizing' || a.status === 'partial';
}

export function voiceLabel(v: Voice | undefined): string {
  if (!v) return '';
  return `${v.name} · ${v.accent === 'british' ? '英音' : '美音'}`;
}

/** 0:00 / 1:02:03 */
export function fmtTime(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${s}` : `${m}:${s}`;
}

/** 二进制单位（与缓存上限选项 1 / 2 / 5 GB = 1024³ 字节一致）。 */
export function fmtBytes(n: number): string {
  const K = 1024;
  if (n < K * K) return `${Math.round(n / K)} KB`;
  if (n < K * K * K) return `${(n / K / K).toFixed(1)} MB`;
  const g = n / K / K / K;
  return `${Number.isInteger(Math.round(g * 100) / 100) ? g.toFixed(0) : g.toFixed(2)} GB`;
}

/** 在时间轴上定位当前句 / 词。 */
export function locate(
  t: Timeline | null,
  ms: number,
  hint: number | null,
): { sentence: number | null; word: number | null } {
  if (!t) return { sentence: hint, word: null };
  let sentence: number | null = null;
  for (const s of t.sentences) {
    if (!s.ready) break;
    if (s.start_ms <= ms) sentence = s.index;
    else break;
  }
  if (sentence == null) return { sentence: hint ?? (t.sentences[0]?.ready ? 0 : null), word: null };
  const words: WordSpan[] = t.sentences[sentence].words;
  let word: number | null = null;
  for (let i = 0; i < words.length; i++) {
    if (words[i].start_ms <= ms) word = i;
    else break;
  }
  return { sentence, word };
}

export const store = new Store();
