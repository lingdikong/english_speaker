//! SQLite 存储：资料库 + 缓存池索引。`bundled` 特性把 SQLite 源码编进产物，五个平台行为一致。
//!
//! 表结构：
//! - `articles`       文章与原文；`pinned` 表示“保留下载”
//! - `sentences`      每句文本；合成后写入块位置（byte_offset/len, start_frame/frames）与时间轴
//! - `cache_entries`  缓存池：每篇一条，状态 / 大小 / 最近访问时间（LRU 依据）
//! - `progress`       播放进度
//! - `settings`       键值设置（缓存上限等）

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use es_core::model::*;
use es_core::ports::Storage;
use es_core::{CoreError, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};

/// 按顺序执行的迁移脚本；下标 + 1 即 `PRAGMA user_version`。
const MIGRATIONS: &[&str] = &[r#"
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
"#];

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
}
