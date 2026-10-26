// 与核心层 serde JSON 一一对应（es-core model.rs / events.rs / text/connected.rs）。

export type SourceKind = 'paste' | 'file' | 'share';
export type AudioStatus = 'not_cached' | 'queued' | 'synthesizing' | 'partial' | 'ready' | 'failed';
export type PlayerStatus = 'idle' | 'playing' | 'buffering' | 'paused' | 'ended';

export interface ArticleSummary {
  id: string;
  title: string;
  source: SourceKind;
  source_ref: string | null;
  created_at_ms: number;
  voice_id: string | null;
  pinned: boolean;
  status: AudioStatus;
  ready_sentences: number;
  total_sentences: number;
  cached_bytes: number;
  duration_ms: number;
  progress_ms: number | null;
}

/** 分类（类似歌单）：一篇文章可以同时在多个分类里。 */
export interface Collection {
  id: string;
  name: string;
  created_at_ms: number;
  /** 按加入时间先后（新加入的在后）。 */
  article_ids: string[];
}

export interface Article {
  id: string;
  title: string;
  source: SourceKind;
  source_ref: string | null;
  body: string;
  created_at_ms: number;
  voice_id: string | null;
  pinned: boolean;
  sentence_count: number;
  estimated_ms: number;
}

export type Link =
  | 'consonant_vowel'
  | 'glide_j'
  | 'glide_w'
  | 'linking_r'
  | 'same_sound'
  | 'flap'
  | 'assimilation';

export interface WordNote {
  ipa: string;
  link: Link | null;
  weak_form: string | null;
  flap: boolean;
  elided: boolean;
}

export interface WordSpan {
  text: string;
  char_start: number;
  char_end: number;
  start_ms: number;
  end_ms: number;
  notes?: WordNote;
}

export interface SentenceSpan {
  index: number;
  paragraph: number;
  text: string;
  ready: boolean;
  start_ms: number;
  end_ms: number;
  words: WordSpan[];
}

export interface Timeline {
  sentences: SentenceSpan[];
  complete: boolean;
}

export interface Voice {
  id: string;
  name: string;
  accent: 'american' | 'british';
  gender: 'female' | 'male';
}

export interface PlayerState {
  article_id: string | null;
  status: PlayerStatus;
  position_ms: number;
  buffered_ms: number;
  duration_ms: number;
  duration_final: boolean;
  rate: number;
}

export interface CacheStats {
  used_bytes: number;
  limit_bytes: number;
  pinned_bytes: number;
  entries: number;
}

// ---------- 翻译与查词（es-core model.rs “翻译与查词”） ----------

/** fast = 机器翻译（内置），smart = 本地大模型（按需下载）。 */
export type TranslationEngine = 'fast' | 'smart';

export interface SentenceTranslation {
  index: number;
  text: string;
  engine: TranslationEngine;
}

export interface DictSense {
  pos: string;
  meaning: string;
}

export interface DictEntry {
  word: string;
  query: string;
  inflection: string | null;
  phonetic: string;
  senses: DictSense[];
  tags: string[];
  collins: number;
  oxford: boolean;
  frequency: number | null;
  forms: { kind: string; word: string }[];
}

export interface WordInsight {
  word: string;
  pos: string;
  meaning: string;
  collocations: string[];
  examples: { en: string; zh: string }[];
}

export interface SentenceAnalysis {
  translation: string;
  structure: string;
  points: { text: string; note: string }[];
}

export interface GlossaryTerm {
  term: string;
  translation: string;
  created_at_ms: number;
}

export interface VocabEntry {
  word: string;
  phonetic: string;
  meaning: string;
  article_id: string | null;
  article_title: string | null;
  sentence_index: number | null;
  sentence: string;
  added_at_ms: number;
}

export type LlmState = 'not_downloaded' | 'downloading' | 'downloaded' | 'failed';

export interface TranslatorStatus {
  dictionary: boolean;
  fast: boolean;
  smart_enabled: boolean;
  active: TranslationEngine | null;
  llm: {
    state: LlmState;
    model_name: string;
    downloaded_bytes: number;
    total_bytes: number;
    loaded: boolean;
    error: string | null;
  };
}

export type CoreEvent =
  | { type: 'library_changed' }
  | { type: 'synthesis_progress'; article_id: string; ready: number; total: number; sentence: SentenceSpan }
  | { type: 'synthesis_finished'; article_id: string; duration_ms: number }
  | { type: 'synthesis_failed'; article_id: string; message: string }
  | { type: 'cache_changed'; stats: CacheStats }
  | { type: 'player_state_changed'; state: PlayerState }
  | {
      type: 'player_position';
      position_ms: number;
      buffered_ms: number;
      sentence: number | null;
      word: number | null;
    }
  | { type: 'translation_progress'; article_id: string; translation: SentenceTranslation; translated: number; total: number }
  | { type: 'translations_changed'; article_id: string }
  | {
      type: 'word_insight_ready';
      article_id: string;
      sentence: number;
      word: number;
      insight: WordInsight | null;
      error: string | null;
    }
  | {
      type: 'sentence_analysis_ready';
      article_id: string;
      sentence: number;
      analysis: SentenceAnalysis | null;
      error: string | null;
    }
  | { type: 'translator_changed'; status: TranslatorStatus }
  | { type: 'vocabulary_changed' };

/** 宿主信息（`host_info`）：音量、变速能力等宿主侧能力。 */
export interface HostInfo {
  host: string;
  /** 宿主系统（Rust `std::env::consts::OS`）：macos / windows / linux；开发宿主可能没有 */
  os?: string;
  volume: number;
  rate_supported: boolean;
  engine: string;
}
