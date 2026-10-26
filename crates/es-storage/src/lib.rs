//! SQLite 存储：资料库 + 缓存池索引。`bundled` 特性把 SQLite 源码编进产物，五个平台行为一致。
//!
//! 表结构：
//! - `articles`       文章与原文；`pinned` 表示“保留下载”
//! - `sentences`      每句文本；合成后写入块位置（byte_offset/len, start_frame/frames）与时间轴
//! - `cache_entries`  缓存池：每篇一条，状态 / 大小 / 最近访问时间（LRU 依据）
//! - `progress`       播放进度
//! - `settings`       键值设置（缓存上限等）
//! - `collections`    分类（类似歌单）；`collection_articles` 记录文章与分类的多对多关系
//! - `translations`   每句译文，按引擎（fast / smart）各一条
//! - `ai_notes`       智能解析缓存（点词举一反三 / 句子解析，JSON）
//! - `glossary`       用户术语表
//! - `vocabulary`     生词本；文章删除后保留（出处置空）

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use es_core::model::*;
use es_core::ports::Storage;
use es_core::{CoreError, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 按顺序执行的迁移脚本；下标 + 1 即 `PRAGMA user_version`。
const MIGRATIONS: &[&str] = &[
    r#"
CREATE TABLE articles (
    id             TEXT PRIMARY KEY,
    title          TEXT NOT NULL,
    source         TEXT NOT NULL,
    source_ref     TEXT,
    body           TEXT NOT NULL,
    created_at_ms  INTEGER NOT NULL,
    voice_id       TEXT,
    pinned         INTEGER NOT NULL DEFAULT 0,
    sentence_count INTEGER NOT NULL,
    estimated_ms   INTEGER NOT NULL
);
CREATE TABLE sentences (
    article_id  TEXT NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    idx         INTEGER NOT NULL,
    paragraph   INTEGER NOT NULL,
    text        TEXT NOT NULL,
    byte_offset INTEGER,
    byte_len    INTEGER,
    start_frame INTEGER,
    frames      INTEGER,
    start_ms    INTEGER,
    end_ms      INTEGER,
    words_json  TEXT,
    PRIMARY KEY (article_id, idx)
) WITHOUT ROWID;
CREATE TABLE cache_entries (
    article_id      TEXT PRIMARY KEY REFERENCES articles(id) ON DELETE CASCADE,
    voice_id        TEXT NOT NULL,
    codec           TEXT NOT NULL,
    file_name       TEXT NOT NULL,
    sample_rate     INTEGER NOT NULL,
    state           TEXT NOT NULL,
    error           TEXT,
    ready_sentences INTEGER NOT NULL DEFAULT 0,
    total_sentences INTEGER NOT NULL,
    ready_frames    INTEGER NOT NULL DEFAULT 0,
    byte_size       INTEGER NOT NULL DEFAULT 0,
    created_at_ms   INTEGER NOT NULL,
    last_access_ms  INTEGER NOT NULL
);
CREATE INDEX cache_entries_lru ON cache_entries(last_access_ms);
CREATE TABLE progress (
    article_id    TEXT PRIMARY KEY REFERENCES articles(id) ON DELETE CASCADE,
    position_ms   INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#,
    r#"
CREATE TABLE collections (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);
CREATE TABLE collection_articles (
    collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    article_id    TEXT NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    added_at_ms   INTEGER NOT NULL,
    PRIMARY KEY (collection_id, article_id)
) WITHOUT ROWID;
CREATE INDEX collection_articles_article ON collection_articles(article_id);
"#,
    r#"
CREATE TABLE translations (
    article_id    TEXT NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    idx           INTEGER NOT NULL,
    engine        TEXT NOT NULL,
    text          TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (article_id, idx, engine)
) WITHOUT ROWID;
CREATE TABLE ai_notes (
    article_id    TEXT NOT NULL REFERENCES articles(id) ON DELETE CASCADE,
    idx           INTEGER NOT NULL,
    word          INTEGER NOT NULL,  -- -1：整句解析
    json          TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    PRIMARY KEY (article_id, idx, word)
) WITHOUT ROWID;
CREATE TABLE glossary (
    term          TEXT PRIMARY KEY COLLATE NOCASE,
    translation   TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
);
CREATE TABLE vocabulary (
    word         TEXT PRIMARY KEY,
    phonetic     TEXT NOT NULL,
    meaning      TEXT NOT NULL,
    article_id   TEXT REFERENCES articles(id) ON DELETE SET NULL,
    sentence_idx INTEGER,
    sentence     TEXT NOT NULL,
    added_at_ms  INTEGER NOT NULL
);
CREATE INDEX vocabulary_added ON vocabulary(added_at_ms);
"#,
];

const CLEAR_CHUNKS: &str =
    "UPDATE sentences SET byte_offset = NULL, byte_len = NULL, start_frame = NULL,
     frames = NULL, start_ms = NULL, end_ms = NULL, words_json = NULL WHERE article_id = ?1";

const ENTRY_COLUMNS: &str = "c.article_id, c.voice_id, c.codec, c.file_name, c.sample_rate, c.state, c.error,
     c.ready_sentences, c.total_sentences, c.ready_frames, c.byte_size, c.created_at_ms, c.last_access_ms";

pub struct SqliteStorage {
    conn: Mutex<Connection>,
}

fn db_err(e: rusqlite::Error) -> CoreError {
    CoreError::Storage(e.to_string())
}

fn json_err(e: serde_json::Error) -> CoreError {
    CoreError::Storage(e.to_string())
}

impl SqliteStorage {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path).map_err(db_err)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory().map_err(db_err)?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;",
        )
        .map_err(db_err)?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(db_err)?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = conn.transaction().map_err(db_err)?;
            tx.execute_batch(sql).map_err(db_err)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)
                .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn query_entries(
        &self,
        where_clause: &str,
        args: &[&dyn rusqlite::ToSql],
    ) -> Result<Vec<CacheEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT {ENTRY_COLUMNS} FROM cache_entries c JOIN articles a ON a.id = c.article_id {where_clause}"
            ))
            .map_err(db_err)?;
        let rows = stmt.query_map(args, entry_from_row).map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }
}

fn source_to_str(s: SourceKind) -> &'static str {
    match s {
        SourceKind::Paste => "paste",
        SourceKind::File => "file",
        SourceKind::Share => "share",
    }
}

fn source_from_str(s: &str) -> SourceKind {
    match s {
        "file" => SourceKind::File,
        "share" => SourceKind::Share,
        _ => SourceKind::Paste,
    }
}

fn status_from_sql(s: Option<String>) -> AudioStatus {
    s.as_deref()
        .and_then(AudioStatus::parse)
        .unwrap_or(AudioStatus::NotCached)
}

fn entry_from_row(r: &Row<'_>) -> rusqlite::Result<CacheEntry> {
    Ok(CacheEntry {
        article_id: r.get(0)?,
        voice_id: r.get(1)?,
        codec: r.get(2)?,
        file_name: r.get(3)?,
        sample_rate: r.get(4)?,
        state: status_from_sql(r.get(5)?),
        error: r.get(6)?,
        ready_sentences: r.get(7)?,
        total_sentences: r.get(8)?,
        ready_frames: r.get::<_, i64>(9)? as u64,
        byte_size: r.get::<_, i64>(10)? as u64,
        created_at_ms: r.get(11)?,
        last_access_ms: r.get(12)?,
    })
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Storage for SqliteStorage {
    fn insert_article(&self, a: &Article, sentences: &[SentenceSpan]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(db_err)?;
        tx.execute(
            "INSERT INTO articles (id, title, source, source_ref, body, created_at_ms, voice_id, pinned,
                                   sentence_count, estimated_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                a.id,
                a.title,
                source_to_str(a.source),
                a.source_ref,
                a.body,
                a.created_at_ms,
                a.voice_id,
                a.pinned,
                a.sentence_count,
                a.estimated_ms as i64
            ],
        )
        .map_err(db_err)?;
        {
            let mut stmt = tx
                .prepare("INSERT INTO sentences (article_id, idx, paragraph, text) VALUES (?1, ?2, ?3, ?4)")
                .map_err(db_err)?;
            for s in sentences {
                stmt.execute(params![a.id, s.index, s.paragraph, s.text])
                    .map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)
    }

    fn get_article(&self, id: &str) -> Result<Option<Article>> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id, title, source, source_ref, body, created_at_ms, voice_id, pinned,
                        sentence_count, estimated_ms
                 FROM articles WHERE id = ?1",
                [id],
                |r| {
                    Ok(Article {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        source: source_from_str(&r.get::<_, String>(2)?),
                        source_ref: r.get(3)?,
                        body: r.get(4)?,
                        created_at_ms: r.get(5)?,
                        voice_id: r.get(6)?,
                        pinned: r.get(7)?,
                        sentence_count: r.get(8)?,
                        estimated_ms: r.get::<_, i64>(9)? as u64,
                    })
                },
            )
            .optional()
            .map_err(db_err)
    }

    fn list_articles(&self) -> Result<Vec<ArticleSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT a.id, a.title, a.source, a.source_ref, a.created_at_ms, COALESCE(c.voice_id, a.voice_id), a.pinned,
                        a.sentence_count, a.estimated_ms,
                        c.state, c.ready_sentences, c.ready_frames, c.sample_rate, c.byte_size,
                        p.position_ms
                 FROM articles a
                 LEFT JOIN cache_entries c ON c.article_id = a.id
                 LEFT JOIN progress p ON p.article_id = a.id
                 ORDER BY a.created_at_ms DESC",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                let status = status_from_sql(r.get(9)?);
                let total: u32 = r.get(7)?;
                let ready: u32 = r.get::<_, Option<u32>>(10)?.unwrap_or(0);
                let ready_frames = r.get::<_, Option<i64>>(11)?.unwrap_or(0) as u64;
                let rate = r.get::<_, Option<u32>>(12)?.unwrap_or(1).max(1) as u64;
                let estimated = r.get::<_, i64>(8)? as u64;
                Ok(ArticleSummary {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    source: source_from_str(&r.get::<_, String>(2)?),
                    source_ref: r.get(3)?,
                    created_at_ms: r.get(4)?,
                    voice_id: r.get(5)?,
                    pinned: r.get(6)?,
                    status,
                    ready_sentences: ready,
                    total_sentences: total,
                    cached_bytes: r.get::<_, Option<i64>>(13)?.unwrap_or(0) as u64,
                    duration_ms: blended_duration_ms(
                        estimated,
                        ready_frames * 1000 / rate,
                        ready,
                        total,
                        status == AudioStatus::Ready,
                    ),
                    progress_ms: r.get::<_, Option<i64>>(14)?.map(|v| v as u64),
                })
            })
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    fn delete_article(&self, id: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM articles WHERE id = ?1", [id])
            .map_err(db_err)?;
        Ok(())
    }

    fn set_article_voice(&self, id: &str, voice_id: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE articles SET voice_id = ?2 WHERE id = ?1",
                params![id, voice_id],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn set_pinned(&self, id: &str, pinned: bool) -> Result<()> {
        let n = self
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE articles SET pinned = ?2 WHERE id = ?1",
                params![id, pinned],
            )
            .map_err(db_err)?;
        if n == 0 {
            return Err(CoreError::NotFound(id.into()));
        }
        Ok(())
    }

    fn list_collections(&self) -> Result<Vec<Collection>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, name, created_at_ms FROM collections ORDER BY created_at_ms, rowid",
            )
            .map_err(db_err)?;
        let mut list = stmt
            .query_map([], |r| {
                Ok(Collection {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    created_at_ms: r.get(2)?,
                    article_ids: Vec::new(),
                })
            })
            .map_err(db_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db_err)?;
        let mut stmt = conn
            .prepare(
                "SELECT collection_id, article_id FROM collection_articles
                 ORDER BY added_at_ms, article_id",
            )
            .map_err(db_err)?;
        let members = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(db_err)?;
        for m in members {
            let (cid, aid) = m.map_err(db_err)?;
            if let Some(c) = list.iter_mut().find(|c| c.id == cid) {
                c.article_ids.push(aid);
            }
        }
        Ok(list)
    }

    fn insert_collection(&self, c: &Collection) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO collections (id, name, created_at_ms) VALUES (?1, ?2, ?3)",
                params![c.id, c.name, c.created_at_ms],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn rename_collection(&self, id: &str, name: &str) -> Result<()> {
        let n = self
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE collections SET name = ?2 WHERE id = ?1",
                params![id, name],
            )
            .map_err(db_err)?;
        if n == 0 {
            return Err(CoreError::NotFound(id.into()));
        }
        Ok(())
    }

    fn delete_collection(&self, id: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM collections WHERE id = ?1", [id])
            .map_err(db_err)?;
        Ok(())
    }

    fn add_to_collection(
        &self,
        collection_id: &str,
        article_id: &str,
        added_at_ms: i64,
    ) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT OR IGNORE INTO collection_articles (collection_id, article_id, added_at_ms)
                 VALUES (?1, ?2, ?3)",
                params![collection_id, article_id, added_at_ms],
            )
            .map_err(|e| match e {
                // 外键失败：分类或文章不存在
                rusqlite::Error::SqliteFailure(f, _)
                    if f.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    CoreError::NotFound(format!("{collection_id} / {article_id}"))
                }
                e => db_err(e),
            })?;
        Ok(())
    }

    fn remove_from_collection(&self, collection_id: &str, article_id: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM collection_articles WHERE collection_id = ?1 AND article_id = ?2",
                params![collection_id, article_id],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn list_sentences(&self, article_id: &str) -> Result<Vec<SentenceRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT idx, paragraph, text, byte_offset, byte_len, start_frame, frames,
                        start_ms, end_ms, words_json
                 FROM sentences WHERE article_id = ?1 ORDER BY idx",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([article_id], |r| {
                let offset: Option<i64> = r.get(3)?;
                let chunk = match offset {
                    Some(off) => Some(Chunk {
                        byte_offset: off as u64,
                        byte_len: r.get::<_, i64>(4)? as u64,
                        start_frame: r.get::<_, i64>(5)? as u64,
                        frames: r.get::<_, i64>(6)? as u64,
                    }),
                    None => None,
                };
                Ok((
                    r.get::<_, u32>(0)?,
                    r.get::<_, u32>(1)?,
                    r.get::<_, String>(2)?,
                    chunk,
                    r.get::<_, Option<i64>>(7)?,
                    r.get::<_, Option<i64>>(8)?,
                    r.get::<_, Option<String>>(9)?,
                ))
            })
            .map_err(db_err)?;
        let mut out = Vec::new();
        for row in rows {
            let (index, paragraph, text, chunk, start, end, words) = row.map_err(db_err)?;
            let words = match words {
                Some(json) => serde_json::from_str(&json).map_err(json_err)?,
                None => Vec::new(),
            };
            out.push(SentenceRecord {
                span: SentenceSpan {
                    index,
                    paragraph,
                    text,
                    ready: chunk.is_some(),
                    start_ms: start.unwrap_or(0) as u64,
                    end_ms: end.unwrap_or(0) as u64,
                    words,
                },
                chunk,
            });
        }
        Ok(out)
    }

    fn get_cache_entry(&self, article_id: &str) -> Result<Option<CacheEntry>> {
        Ok(self
            .query_entries("WHERE c.article_id = ?1", &[&article_id])?
            .into_iter()
            .next())
    }

    fn list_cache_entries(&self) -> Result<Vec<CacheEntry>> {
        self.query_entries("ORDER BY c.last_access_ms", &[])
    }

    fn create_cache_entry(&self, e: &CacheEntry) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(db_err)?;
        tx.execute(
            "INSERT OR REPLACE INTO cache_entries
               (article_id, voice_id, codec, file_name, sample_rate, state, error, ready_sentences,
                total_sentences, ready_frames, byte_size, created_at_ms, last_access_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                e.article_id,
                e.voice_id,
                e.codec,
                e.file_name,
                e.sample_rate,
                e.state.as_str(),
                e.error,
                e.ready_sentences,
                e.total_sentences,
                e.ready_frames as i64,
                e.byte_size as i64,
                e.created_at_ms,
                e.last_access_ms
            ],
        )
        .map_err(db_err)?;
        tx.execute(CLEAR_CHUNKS, [&e.article_id]).map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    fn set_cache_state(
        &self,
        article_id: &str,
        state: AudioStatus,
        error: Option<&str>,
    ) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE cache_entries SET state = ?2, error = ?3 WHERE article_id = ?1",
                params![article_id, state.as_str(), error],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn commit_chunk(&self, entry: &CacheEntry, chunk: &Chunk, span: &SentenceSpan) -> Result<()> {
        let words = serde_json::to_string(&span.words).map_err(json_err)?;
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(db_err)?;
        let updated = tx
            .execute(
                "UPDATE cache_entries
                 SET ready_sentences = ready_sentences + 1, ready_frames = ?3, byte_size = byte_size + ?4
                 WHERE article_id = ?1 AND file_name = ?2",
                params![
                    entry.article_id,
                    entry.file_name,
                    chunk.end_frame() as i64,
                    chunk.byte_len as i64
                ],
            )
            .map_err(db_err)?;
        if updated == 0 {
            return Err(CoreError::NotFound(format!(
                "cache entry {} was replaced or removed",
                entry.file_name
            )));
        }
        tx.execute(
            "UPDATE sentences
             SET byte_offset = ?3, byte_len = ?4, start_frame = ?5, frames = ?6,
                 start_ms = ?7, end_ms = ?8, words_json = ?9
             WHERE article_id = ?1 AND idx = ?2",
            params![
                entry.article_id,
                span.index,
                chunk.byte_offset as i64,
                chunk.byte_len as i64,
                chunk.start_frame as i64,
                chunk.frames as i64,
                span.start_ms as i64,
                span.end_ms as i64,
                words
            ],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    fn touch_cache_entry(&self, article_id: &str, now_ms: i64) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE cache_entries SET last_access_ms = ?2 WHERE article_id = ?1",
                params![article_id, now_ms],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn delete_cache_entry(&self, article_id: &str) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(db_err)?;
        tx.execute(
            "DELETE FROM cache_entries WHERE article_id = ?1",
            [article_id],
        )
        .map_err(db_err)?;
        tx.execute(CLEAR_CHUNKS, [article_id]).map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    fn eviction_candidates(&self) -> Result<Vec<CacheEntry>> {
        self.query_entries(
            "WHERE a.pinned = 0 AND c.byte_size > 0 ORDER BY c.last_access_ms ASC",
            &[],
        )
    }

    fn cache_usage(&self) -> Result<(u64, u64, u32)> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT COALESCE(SUM(c.byte_size), 0),
                        COALESCE(SUM(CASE WHEN a.pinned THEN c.byte_size ELSE 0 END), 0),
                        COUNT(*)
                 FROM cache_entries c JOIN articles a ON a.id = c.article_id",
                [],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)? as u64,
                        r.get::<_, i64>(1)? as u64,
                        r.get(2)?,
                    ))
                },
            )
            .map_err(db_err)
    }

    fn save_progress(&self, article_id: &str, position_ms: u64) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT OR REPLACE INTO progress (article_id, position_ms, updated_at_ms)
                 SELECT ?1, ?2, ?3 WHERE EXISTS (SELECT 1 FROM articles WHERE id = ?1)",
                params![article_id, position_ms as i64, now_ms()],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn get_progress(&self, article_id: &str) -> Result<Option<u64>> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT position_ms FROM progress WHERE article_id = ?1",
                [article_id],
                |r| r.get::<_, i64>(0),
            )
            .optional()
            .map(|v| v.map(|v| v as u64))
            .map_err(db_err)
    }

    fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .lock()
            .unwrap()
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(db_err)
    }

    fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn put_translation(
        &self,
        article_id: &str,
        index: u32,
        engine: TranslationEngine,
        text: &str,
    ) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT OR REPLACE INTO translations (article_id, idx, engine, text, created_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![article_id, index, engine.as_str(), text, now_ms()],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn list_translations(&self, article_id: &str) -> Result<Vec<SentenceTranslation>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT idx, engine, text FROM translations WHERE article_id = ?1 ORDER BY idx, engine",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([article_id], |r| {
                Ok((
                    r.get::<_, u32>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(db_err)?;
        let mut out = Vec::new();
        for row in rows {
            let (index, engine, text) = row.map_err(db_err)?;
            if let Some(engine) = TranslationEngine::parse(&engine) {
                out.push(SentenceTranslation {
                    index,
                    text,
                    engine,
                });
            }
        }
        Ok(out)
    }

    fn invalidate_translations(&self, term: &str) -> Result<Vec<ArticleId>> {
        let pattern = format!(
            "%{}%",
            term.trim()
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        );
        // 子串匹配（比整词匹配宽）：多重翻几句没有坏处
        const HIT: &str = "SELECT article_id, idx FROM sentences WHERE text LIKE ?1 ESCAPE '\\'";
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(db_err)?;
        let ids = {
            let mut stmt = tx
                .prepare(&format!(
                    "SELECT DISTINCT t.article_id FROM translations t
                     WHERE (t.article_id, t.idx) IN ({HIT})"
                ))
                .map_err(db_err)?;
            let rows = stmt
                .query_map([&pattern], |r| r.get::<_, String>(0))
                .map_err(db_err)?;
            rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)?
        };
        tx.execute(
            &format!("DELETE FROM translations WHERE (article_id, idx) IN ({HIT})"),
            [&pattern],
        )
        .map_err(db_err)?;
        tx.execute(
            &format!("DELETE FROM ai_notes WHERE word = -1 AND (article_id, idx) IN ({HIT})"),
            [&pattern],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        Ok(ids)
    }

    fn get_ai_note(
        &self,
        article_id: &str,
        sentence: u32,
        word: Option<u32>,
    ) -> Result<Option<String>> {
        let word = word.map_or(-1, i64::from);
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT json FROM ai_notes WHERE article_id = ?1 AND idx = ?2 AND word = ?3",
                params![article_id, sentence, word],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)
    }

    fn put_ai_note(
        &self,
        article_id: &str,
        sentence: u32,
        word: Option<u32>,
        json: &str,
    ) -> Result<()> {
        let word = word.map_or(-1, i64::from);
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT OR REPLACE INTO ai_notes (article_id, idx, word, json, created_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![article_id, sentence, word, json, now_ms()],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn list_glossary(&self) -> Result<Vec<GlossaryTerm>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT term, translation, created_at_ms FROM glossary ORDER BY term COLLATE NOCASE")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(GlossaryTerm {
                    term: r.get(0)?,
                    translation: r.get(1)?,
                    created_at_ms: r.get(2)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    fn upsert_glossary(&self, t: &GlossaryTerm) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(db_err)?;
        // 主键不区分大小写：改写法（app → App）时先删旧行，保留用户最新的写法
        tx.execute("DELETE FROM glossary WHERE term = ?1", [&t.term])
            .map_err(db_err)?;
        tx.execute(
            "INSERT INTO glossary (term, translation, created_at_ms) VALUES (?1, ?2, ?3)",
            params![t.term, t.translation, t.created_at_ms],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }

    fn delete_glossary(&self, term: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM glossary WHERE term = ?1", [term])
            .map_err(db_err)?;
        Ok(())
    }

    fn list_vocab(&self) -> Result<Vec<VocabEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT v.word, v.phonetic, v.meaning, v.article_id, a.title, v.sentence_idx,
                        v.sentence, v.added_at_ms
                 FROM vocabulary v LEFT JOIN articles a ON a.id = v.article_id
                 ORDER BY v.added_at_ms DESC",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                let article_id: Option<String> = r.get(3)?;
                Ok(VocabEntry {
                    word: r.get(0)?,
                    phonetic: r.get(1)?,
                    meaning: r.get(2)?,
                    article_title: r.get(4)?,
                    sentence_index: if article_id.is_some() {
                        r.get(5)?
                    } else {
                        None
                    },
                    article_id,
                    sentence: r.get(6)?,
                    added_at_ms: r.get(7)?,
                })
            })
            .map_err(db_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(db_err)
    }

    fn upsert_vocab(&self, v: &VocabEntry) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT OR REPLACE INTO vocabulary
                   (word, phonetic, meaning, article_id, sentence_idx, sentence, added_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    v.word,
                    v.phonetic,
                    v.meaning,
                    v.article_id,
                    v.sentence_index,
                    v.sentence,
                    v.added_at_ms
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    fn delete_vocab(&self, word: &str) -> Result<()> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM vocabulary WHERE word = ?1", [word])
            .map_err(db_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn article(id: &str, created: i64) -> (Article, Vec<SentenceSpan>) {
        let spans = (0..2)
            .map(|i| SentenceSpan {
                index: i,
                paragraph: 0,
                text: format!("Sentence {i}."),
                ready: false,
                start_ms: 0,
                end_ms: 0,
                words: vec![],
            })
            .collect::<Vec<_>>();
        let a = Article {
            id: id.into(),
            title: "T".into(),
            source: SourceKind::Paste,
            source_ref: None,
            body: "Sentence 0. Sentence 1.".into(),
            created_at_ms: created,
            voice_id: None,
            pinned: false,
            sentence_count: 2,
            estimated_ms: 2000,
        };
        (a, spans)
    }

    fn entry(id: &str, file: &str, access: i64) -> CacheEntry {
        CacheEntry {
            article_id: id.into(),
            voice_id: "v".into(),
            codec: "pcm_s16le".into(),
            file_name: file.into(),
            sample_rate: 1000,
            state: AudioStatus::Synthesizing,
            error: None,
            ready_sentences: 0,
            total_sentences: 2,
            ready_frames: 0,
            byte_size: 0,
            created_at_ms: access,
            last_access_ms: access,
        }
    }

    fn chunk(offset: u64, start: u64) -> Chunk {
        Chunk {
            byte_offset: offset,
            byte_len: 100,
            start_frame: start,
            frames: 50,
        }
    }

    fn ready_span(index: u32) -> SentenceSpan {
        SentenceSpan {
            index,
            paragraph: 0,
            text: String::new(),
            ready: true,
            start_ms: index as u64 * 50,
            end_ms: index as u64 * 50 + 40,
            words: vec![],
        }
    }

    #[test]
    fn chunks_and_cache_accounting() {
        let s = SqliteStorage::open_in_memory().unwrap();
        let (a, spans) = article("a1", 1);
        s.insert_article(&a, &spans).unwrap();
        let e = entry("a1", "a1-x.pcm", 1);
        s.create_cache_entry(&e).unwrap();

        s.commit_chunk(&e, &chunk(0, 0), &ready_span(0)).unwrap();
        let sentences = s.list_sentences("a1").unwrap();
        assert!(sentences[0].span.ready && !sentences[1].span.ready);
        assert_eq!(sentences[0].chunk, Some(chunk(0, 0)));

        let summary = &s.list_articles().unwrap()[0];
        assert_eq!(summary.status, AudioStatus::Synthesizing);
        assert_eq!((summary.ready_sentences, summary.cached_bytes), (1, 100));
        // 50 帧 @1kHz = 50ms 已合成 + 剩余一半的估算 1000ms
        assert_eq!(summary.duration_ms, 1050);

        // 旧文件名的提交被拒绝（换音色后旧任务的残留写入）
        let stale = entry("a1", "a1-old.pcm", 1);
        assert!(s
            .commit_chunk(&stale, &chunk(100, 50), &ready_span(1))
            .is_err());

        s.delete_cache_entry("a1").unwrap();
        assert!(s
            .list_sentences("a1")
            .unwrap()
            .iter()
            .all(|x| x.chunk.is_none()));
        assert_eq!(s.list_articles().unwrap()[0].status, AudioStatus::NotCached);
    }

    #[test]
    fn lru_candidates_skip_pinned_and_empty() {
        let s = SqliteStorage::open_in_memory().unwrap();
        for (id, access) in [("old", 1), ("new", 3), ("pinned", 0), ("empty", 2)] {
            let (a, spans) = article(id, access);
            s.insert_article(&a, &spans).unwrap();
            let e = entry(id, &format!("{id}.pcm"), access);
            s.create_cache_entry(&e).unwrap();
            if id != "empty" {
                s.commit_chunk(&e, &chunk(0, 0), &ready_span(0)).unwrap();
            }
        }
        s.set_pinned("pinned", true).unwrap();
        let ids: Vec<_> = s
            .eviction_candidates()
            .unwrap()
            .into_iter()
            .map(|e| e.article_id)
            .collect();
        assert_eq!(ids, vec!["old", "new"]);
        assert_eq!(s.cache_usage().unwrap(), (300, 100, 4));
    }

    #[test]
    fn collections() {
        let s = SqliteStorage::open_in_memory().unwrap();
        for id in ["a1", "a2", "a3"] {
            let (a, spans) = article(id, 1);
            s.insert_article(&a, &spans).unwrap();
        }
        let c = |id: &str, name: &str, t| Collection {
            id: id.into(),
            name: name.into(),
            created_at_ms: t,
            article_ids: Vec::new(),
        };
        s.insert_collection(&c("c2", "Later", 2)).unwrap();
        s.insert_collection(&c("c1", "News", 1)).unwrap();
        s.rename_collection("c1", "Daily news").unwrap();
        assert!(s.rename_collection("nope", "x").is_err());

        // 一篇文章可以同时在多个分类里；分类内按加入时间排序；重复加入不改变顺序
        s.add_to_collection("c1", "a2", 10).unwrap();
        s.add_to_collection("c1", "a1", 20).unwrap();
        s.add_to_collection("c2", "a1", 30).unwrap();
        s.add_to_collection("c1", "a2", 40).unwrap();
        assert!(matches!(
            s.add_to_collection("missing", "a1", 50),
            Err(CoreError::NotFound(_))
        ));
        assert!(matches!(
            s.add_to_collection("c1", "gone", 50),
            Err(CoreError::NotFound(_))
        ));
        let members = |s: &SqliteStorage| {
            s.list_collections()
                .unwrap()
                .into_iter()
                .map(|c| (c.name, c.article_ids))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            members(&s),
            vec![
                (
                    "Daily news".to_string(),
                    vec!["a2".to_string(), "a1".to_string()]
                ),
                ("Later".to_string(), vec!["a1".to_string()]),
            ]
        );

        s.remove_from_collection("c1", "a2").unwrap();
        s.remove_from_collection("c1", "a2").unwrap(); // 不在其中也不报错
                                                       // 删除文章：从所有分类里消失
        s.delete_article("a1").unwrap();
        // 删除分类：文章保留
        s.add_to_collection("c2", "a3", 60).unwrap();
        s.delete_collection("c2").unwrap();
        assert_eq!(
            members(&s),
            vec![("Daily news".to_string(), Vec::<String>::new())]
        );
        assert_eq!(s.list_articles().unwrap().len(), 2);
    }

    #[test]
    fn delete_cascades_and_settings() {
        let s = SqliteStorage::open_in_memory().unwrap();
        let (a, spans) = article("a1", 1);
        s.insert_article(&a, &spans).unwrap();
        s.create_cache_entry(&entry("a1", "f", 1)).unwrap();
        s.save_progress("a1", 500).unwrap();
        s.delete_article("a1").unwrap();
        assert!(s.get_cache_entry("a1").unwrap().is_none());
        assert!(s.list_sentences("a1").unwrap().is_empty());
        s.save_progress("gone", 1).unwrap(); // 已删除的文章不写进度，也不报错
        assert_eq!(s.get_progress("gone").unwrap(), None);

        s.set_setting("cache.limit_bytes", "42").unwrap();
        assert_eq!(
            s.get_setting("cache.limit_bytes").unwrap().as_deref(),
            Some("42")
        );
    }

    #[test]
    fn translations_cache_and_invalidate_by_term() {
        let s = SqliteStorage::open_in_memory().unwrap();
        let (a, spans) = article("a", 1);
        s.insert_article(&a, &spans).unwrap();
        s.put_translation("a", 0, TranslationEngine::Fast, "句子 0。")
            .unwrap();
        s.put_translation("a", 0, TranslationEngine::Smart, "第 0 句。")
            .unwrap();
        s.put_translation("a", 1, TranslationEngine::Fast, "句子 1。")
            .unwrap();
        s.put_ai_note("a", 1, None, "{}").unwrap();
        s.put_ai_note("a", 1, Some(0), "{\"w\":1}").unwrap();
        assert_eq!(s.list_translations("a").unwrap().len(), 3);

        // “Sentence 1.” 命中：只作废第 1 句的译文和整句解析，点词缓存保留
        assert_eq!(s.invalidate_translations("ce 1").unwrap(), ["a"]);
        let left: Vec<_> = s
            .list_translations("a")
            .unwrap()
            .into_iter()
            .map(|t| (t.index, t.engine))
            .collect();
        assert_eq!(
            left,
            [(0, TranslationEngine::Fast), (0, TranslationEngine::Smart)]
        );
        assert!(s.get_ai_note("a", 1, None).unwrap().is_none());
        assert!(s.get_ai_note("a", 1, Some(0)).unwrap().is_some());
        // LIKE 通配符按字面匹配
        assert!(s.invalidate_translations("%").unwrap().is_empty());

        s.delete_article("a").unwrap();
        assert!(s.list_translations("a").unwrap().is_empty());
    }

    #[test]
    fn glossary_is_case_insensitive() {
        let s = SqliteStorage::open_in_memory().unwrap();
        let term = |t: &str, zh: &str| GlossaryTerm {
            term: t.into(),
            translation: zh.into(),
            created_at_ms: 0,
        };
        s.upsert_glossary(&term("app", "应用")).unwrap();
        s.upsert_glossary(&term("App", "应用程序")).unwrap();
        s.upsert_glossary(&term("Token", "令牌")).unwrap();
        let all = s.list_glossary().unwrap();
        assert_eq!(all, [term("App", "应用程序"), term("Token", "令牌")]);
        s.delete_glossary("TOKEN").unwrap();
        assert_eq!(s.list_glossary().unwrap().len(), 1);
    }

    #[test]
    fn vocabulary_survives_article_deletion() {
        let s = SqliteStorage::open_in_memory().unwrap();
        let (a, spans) = article("a", 1);
        s.insert_article(&a, &spans).unwrap();
        let mut v = VocabEntry {
            word: "sentence".into(),
            phonetic: "ˈsentəns".into(),
            meaning: "n. 句子".into(),
            article_id: Some("a".into()),
            article_title: None,
            sentence_index: Some(1),
            sentence: "Sentence 1.".into(),
            added_at_ms: 10,
        };
        s.upsert_vocab(&v).unwrap();
        v.added_at_ms = 20;
        s.upsert_vocab(&v).unwrap();
        let list = s.list_vocab().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].article_title.as_deref(), Some("T"));
        assert_eq!(list[0].added_at_ms, 20);

        s.delete_article("a").unwrap();
        let list = s.list_vocab().unwrap();
        assert_eq!(
            (list[0].article_id.as_deref(), list[0].sentence_index),
            (None, None)
        );
        assert_eq!(list[0].sentence, "Sentence 1.");
        s.delete_vocab("sentence").unwrap();
        assert!(s.list_vocab().unwrap().is_empty());
    }
}
