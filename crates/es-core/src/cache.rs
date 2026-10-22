//! 缓存池：音频文件 + SQL 索引，按容量上限做 LRU 淘汰。
//!
//! - 每篇文章一个只追加的音频文件 `audio/<article_id>.<ext>`；
//! - 每句的块位置、每篇的状态/大小/最近访问时间都记录在 SQL；
//! - 超过上限时按 `last_access_ms` 从旧到新淘汰，跳过：钉住的、正在播放的、正在合成的；
//! - 淘汰只删除音频与块索引，文章（含原文与句子文本）保留，再次播放时重新流式合成。

use std::path::PathBuf;

use crate::app::Shared;
use crate::error::Result;
use crate::events::CoreEvent;
use crate::model::{Article, AudioStatus, CacheEntry, CacheStats};

pub const DEFAULT_LIMIT_BYTES: u64 = 1024 * 1024 * 1024;
pub(crate) const LIMIT_SETTING: &str = "cache.limit_bytes";

impl Shared {
    pub fn audio_path(&self, entry: &CacheEntry) -> PathBuf {
        self.audio_dir.join(&entry.file_name)
    }

    pub fn cache_stats(&self) -> Result<CacheStats> {
        let (used_bytes, pinned_bytes, entries) = self.storage.cache_usage()?;
        Ok(CacheStats {
            used_bytes,
            limit_bytes: self.cache_limit(),
            pinned_bytes,
            entries,
        })
    }

    pub fn cache_limit(&self) -> u64 {
        self.cache_limit.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 为文章新建一条空的缓存记录。文件名带随机后缀：换音色重建时与旧文件互不干扰。
    pub fn new_cache_entry(
        &self,
        article: &Article,
        voice_id: &str,
        state: AudioStatus,
    ) -> Result<CacheEntry> {
        let now = crate::app::now_ms();
        let suffix = &uuid::Uuid::new_v4().simple().to_string()[..8];
        let entry = CacheEntry {
            article_id: article.id.clone(),
            voice_id: voice_id.to_string(),
            codec: self.codec.id().to_string(),
            file_name: format!("{}-{suffix}.{}", article.id, self.codec.extension()),
            sample_rate: self.tts.sample_rate(),
            state,
            error: None,
            ready_sentences: 0,
            total_sentences: article.sentence_count,
            ready_frames: 0,
            byte_size: 0,
            created_at_ms: now,
            last_access_ms: now,
        };
        self.storage.create_cache_entry(&entry)?;
        Ok(entry)
    }

    /// 删除一篇文章的缓存（文件 + 索引）。
    pub fn remove_cache(&self, article_id: &str) -> Result<()> {
        if let Some(entry) = self.storage.get_cache_entry(article_id)? {
            match std::fs::remove_file(self.audio_path(&entry)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            self.storage.delete_cache_entry(article_id)?;
        }
        Ok(())
    }

    /// 超过上限时按 LRU 淘汰。返回被淘汰的文章数。
    pub fn enforce_cache_limit(&self) -> Result<usize> {
        let limit = self.cache_limit();
        let (mut used, _, _) = self.storage.cache_usage()?;
        if used <= limit {
            return Ok(0);
        }
        let protected = [
            self.now_playing.lock().unwrap().clone(),
            self.synthesizing.lock().unwrap().clone(),
        ];
        let mut evicted = 0;
        for entry in self.storage.eviction_candidates()? {
            if used <= limit {
                break;
            }
            if protected.iter().flatten().any(|id| *id == entry.article_id) {
                continue;
            }
            log::info!(
                "cache: evicting {} ({} bytes, last access {})",
                entry.article_id,
                entry.byte_size,
                entry.last_access_ms
            );
            self.remove_cache(&entry.article_id)?;
            used = used.saturating_sub(entry.byte_size);
            evicted += 1;
        }
        if used > limit {
            log::warn!("cache: still {used} bytes over limit {limit} (pinned or in use)");
        }
        if evicted > 0 {
            self.emit(CoreEvent::LibraryChanged);
            self.emit(CoreEvent::CacheChanged {
                stats: self.cache_stats()?,
            });
        }
        Ok(evicted)
    }
}
