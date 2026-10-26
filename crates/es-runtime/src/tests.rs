//! 端到端测试：真实 SQLite + 真实缓存文件 + Mock TTS + 实时速度的 NullSink。

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use es_core::model::{AudioStatus, PlayerStatus, SourceKind, Voice};
use es_core::ports::{SynthOutput, SynthRequest, TtsEngine};
use es_core::testing::{CollectingListener, MockTtsEngine, NullSink};
use es_core::{AppCore, CoreEvent, Result};

use super::*;

/// 可控速度、计数的 TTS：用来制造“合成比播放慢”的场景并验证断点续转不重复合成。
struct SlowTts {
    inner: MockTtsEngine,
    delay: Duration,
    calls: Arc<AtomicUsize>,
}

impl TtsEngine for SlowTts {
    fn id(&self) -> &str {
        "slow-mock"
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn voices(&self) -> Vec<Voice> {
        self.inner.voices()
    }
    fn synthesize(&self, req: &SynthRequest<'_>) -> Result<SynthOutput> {
        std::thread::sleep(self.delay);
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.synthesize(req)
    }
}

struct Harness {
    app: AppCore,
    events: Arc<CollectingListener>,
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("es-runtime-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn harness(
    dir: &std::path::Path,
    delay_ms: u64,
    calls: Arc<AtomicUsize>,
    limit: Option<u64>,
) -> Harness {
    let events = Arc::new(CollectingListener::default());
    let tts = Arc::new(SlowTts {
        inner: MockTtsEngine::default(),
        delay: Duration::from_millis(delay_ms),
        calls,
    });
    let mut config = RuntimeConfig::new(dir.to_path_buf());
    config.cache_limit_bytes = limit;
    let app =
        build_with_engine(config, tts, Arc::new(NullSink::default()), events.clone()).unwrap();
    Harness { app, events }
}

fn wait_for(what: &str, mut f: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !f() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn status(app: &AppCore, id: &str) -> AudioStatus {
    app.list_articles()
        .unwrap()
        .into_iter()
        .find(|a| a.id == id)
        .unwrap()
        .status
}

fn import(app: &AppCore, body: &str) -> String {
    app.import_text(None, body.into(), SourceKind::Paste, None)
        .unwrap()
        .id
}

const ARTICLE: &str = "One two. Three four.\n\nFive six. Seven eight.";

/// 导入即开始转换；合成比播放慢时边听边转，追上时进入缓冲，最终完整播放。
#[test]
fn listen_while_converting() {
    let dir = temp_dir("stream");
    // Mock 每句 0.36–0.6s 音频，合成每句 1.5s：即使 CI 机器调度慢，播放也一定会追上合成
    let h = harness(&dir, 1500, Default::default(), None);
    let id = import(&h.app, ARTICLE);
    assert_eq!(
        h.app.timeline(&id).unwrap().sentences.len(),
        4,
        "原文导入时即完整可见"
    );

    wait_for("first sentence", || {
        h.app.timeline(&id).unwrap().sentences[0].ready
    });
    h.app.play_article(&id).unwrap();
    wait_for("ended", || {
        h.app.player_state().status == PlayerStatus::Ended
    });

    let events = h.events.events.lock().unwrap();
    let position_at = events.iter().position(
        |e| matches!(e, CoreEvent::PlayerPosition { position_ms, .. } if *position_ms > 0),
    );
    let finished_at = events
        .iter()
        .position(|e| matches!(e, CoreEvent::SynthesisFinished { .. }));
    assert!(position_at < finished_at, "合成完成前就已经开始播放");
    assert!(
        events.iter().any(|e| matches!(e,
            CoreEvent::PlayerStateChanged { state } if state.status == PlayerStatus::Buffering)),
        "播放追上合成时进入缓冲"
    );
    drop(events);

    assert_eq!(status(&h.app, &id), AudioStatus::Ready);
    let tl = h.app.timeline(&id).unwrap();
    assert!(tl.complete && tl.sentences.iter().all(|s| s.ready));
    let _ = std::fs::remove_dir_all(&dir);
}

/// App 在合成中途被杀：重启后从断点继续，已合成的句子不重做；
/// 写了一半的块被截断丢弃。
#[test]
fn resume_after_crash() {
    let dir = temp_dir("resume");
    let calls = Arc::new(AtomicUsize::new(0));
    // 每句合成 0.4s：留足时间在合成完之前取消，避免慢机器上直接变成 Ready
    let first = harness(&dir, 400, calls.clone(), None);
    let id = import(&first.app, ARTICLE);
    wait_for("two sentences", || {
        first
            .app
            .timeline(&id)
            .unwrap()
            .sentences
            .iter()
            .filter(|s| s.ready)
            .count()
            >= 2
    });
    first.app.cancel_synthesis(&id);
    wait_for("partial", || {
        status(&first.app, &id) == AudioStatus::Partial
    });
    let done_before = calls.load(Ordering::SeqCst);

    // 模拟崩溃：数据库停在“合成中”，音频文件尾部有写了一半的数据
    let storage = SqliteStorage::open(&dir.join("library.db")).unwrap();
    use es_core::ports::Storage;
    storage
        .set_cache_state(&id, AudioStatus::Synthesizing, None)
        .unwrap();
    let entry = storage.get_cache_entry(&id).unwrap().unwrap();
    let path = dir.join("audio").join(&entry.file_name);
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&[7u8; 1001])
        .unwrap();
    drop(first);

    let second = harness(&dir, 0, calls.clone(), None);
    wait_for("ready after restart", || {
        status(&second.app, &id) == AudioStatus::Ready
    });
    assert_eq!(calls.load(Ordering::SeqCst), 4, "只合成了剩余的句子");
    assert!(done_before >= 2);

    let entry = storage.get_cache_entry(&id).unwrap().unwrap();
    let file_len = std::fs::metadata(&path).unwrap().len();
    assert_eq!(file_len, entry.byte_size, "半截数据已被截断");

    second.app.play_article(&id).unwrap();
    wait_for("ended", || {
        second.app.player_state().status == PlayerStatus::Ended
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// 缓存池超过上限按 LRU 淘汰；钉住的不淘汰；被淘汰的文章再次播放会重新流式合成。
#[test]
fn cache_pool_lru_and_pinning() {
    let dir = temp_dir("lru");
    let h = harness(&dir, 0, Default::default(), None);
    let ids: Vec<String> = [
        "Alpha beta gamma.",
        "Delta epsilon zeta.",
        "Eta theta iota.",
    ]
    .iter()
    .map(|body| {
        let id = import(&h.app, body);
        wait_for("ready", || status(&h.app, &id) == AudioStatus::Ready);
        std::thread::sleep(Duration::from_millis(5)); // 保证最近访问时间不同
        id
    })
    .collect();
    let stats = h.app.cache_stats().unwrap();
    assert_eq!(stats.entries, 3);
    let one = stats.used_bytes / 3;

    h.app.set_pinned(&ids[0], true).unwrap();
    // 上限只够两篇：最旧的未钉住文章（ids[1]）被淘汰
    h.app.set_cache_limit(one * 2 + one / 2).unwrap();
    assert_eq!(status(&h.app, &ids[0]), AudioStatus::Ready, "钉住的保留");
    assert_eq!(status(&h.app, &ids[1]), AudioStatus::NotCached, "LRU 淘汰");
    assert_eq!(status(&h.app, &ids[2]), AudioStatus::Ready);
    assert!(h.app.cache_stats().unwrap().used_bytes <= one * 2 + one / 2);
    assert!(h
        .events
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, CoreEvent::CacheChanged { .. })));

    // 被淘汰的文章仍在资料库，播放时重新合成；正在播放的文章不会被淘汰，
    // 于是这次淘汰的是 ids[2]
    h.app.play_article(&ids[1]).unwrap();
    wait_for("ended", || {
        h.app.player_state().status == PlayerStatus::Ended
    });
    assert_eq!(status(&h.app, &ids[1]), AudioStatus::Ready);
    assert_eq!(status(&h.app, &ids[2]), AudioStatus::NotCached);
    assert_eq!(h.app.list_articles().unwrap().len(), 3);

    // 移除下载 / 清空缓存
    h.app.stop();
    wait_for("idle", || h.app.player_state().status == PlayerStatus::Idle);
    assert_eq!(h.app.clear_cache().unwrap(), 1, "钉住的不清");
    assert_eq!(status(&h.app, &ids[1]), AudioStatus::NotCached);
    h.app.remove_download(&ids[0]).unwrap();
    assert_eq!(h.app.cache_stats().unwrap().used_bytes, 0);
    assert!(
        std::fs::read_dir(dir.join("audio"))
            .unwrap()
            .next()
            .is_none(),
        "文件已删除"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 边听边转时取消/失败合成：读到现有音频末尾后应停止（暂停），
/// 而不是永远停留在缓冲状态（无声但界面显示播放中）。
#[test]
fn stop_when_synthesis_ends_before_audio_does() {
    let dir = temp_dir("stop-on-synth-done");
    let h = harness(&dir, 1500, Default::default(), None);
    let id = import(&h.app, ARTICLE);
    wait_for("first sentence", || {
        h.app.timeline(&id).unwrap().sentences[0].ready
    });
    h.app.play_article(&id).unwrap();
    wait_for("buffering at frontier", || {
        h.app.player_state().status == PlayerStatus::Buffering
    });
    h.app.cancel_synthesis(&id);
    wait_for("paused instead of endless buffering", || {
        h.app.player_state().status == PlayerStatus::Paused
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// 换音色：丢弃旧缓存并用新音色重新合成；删除文章清理文件。
#[test]
fn change_voice_and_delete() {
    let dir = temp_dir("voice");
    let h = harness(&dir, 0, Default::default(), None);
    let id = import(&h.app, ARTICLE);
    wait_for("ready", || status(&h.app, &id) == AudioStatus::Ready);

    h.app.synthesize(&id, Some("mock_m".into())).unwrap();
    wait_for("ready again", || {
        let a = h.app.list_articles().unwrap().remove(0);
        a.status == AudioStatus::Ready && a.voice_id.as_deref() == Some("mock_m")
    });
    assert_eq!(
        std::fs::read_dir(dir.join("audio")).unwrap().count(),
        1,
        "旧文件已删除"
    );
    assert!(h.app.synthesize(&id, Some("nope".into())).is_err());

    h.app.delete_article(&id).unwrap();
    assert!(h.app.list_articles().unwrap().is_empty());
    assert_eq!(std::fs::read_dir(dir.join("audio")).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 分类：重名校验；一篇文章可同时在多个分类里；删除分类不删文章；重启后仍在。
#[test]
fn collections_many_to_many() {
    let dir = temp_dir("collections");
    let (a, c1) = {
        let h = harness(&dir, 0, Default::default(), None);
        let a = import(&h.app, ARTICLE);
        let c1 = h.app.create_collection("  News ").unwrap();
        assert_eq!(c1.name, "News");
        assert!(
            h.app.create_collection("news").is_err(),
            "重名（不区分大小写）"
        );
        assert!(h.app.create_collection("   ").is_err());
        let c2 = h.app.create_collection("Stories").unwrap();
        assert!(h.app.rename_collection(&c2.id, "NEWS").is_err());
        h.app.rename_collection(&c1.id, "News").unwrap(); // 改成自己的名字不算重名

        h.app.add_to_collection(&a, &c1.id).unwrap();
        h.app.add_to_collection(&a, &c2.id).unwrap();
        assert!(h.app.add_to_collection(&a, "missing").is_err());
        let both = h.app.list_collections().unwrap();
        assert!(both.iter().all(|c| c.article_ids == vec![a.clone()]));

        h.app.delete_collection(&c2.id).unwrap();
        assert_eq!(h.app.list_articles().unwrap().len(), 1, "文章保留");
        (a, c1)
    };
    let h = harness(&dir, 0, Default::default(), None);
    let list = h.app.list_collections().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(
        (list[0].id.as_str(), &list[0].article_ids),
        (c1.id.as_str(), &vec![a.clone()])
    );
    h.app.remove_from_collection(&a, &c1.id).unwrap();
    assert!(h.app.list_collections().unwrap()[0].article_ids.is_empty());
    drop(h);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 时间轴中带有连读/弱读标注，并随缓存持久化。
#[test]
fn connected_speech_notes_in_timeline() {
    use es_core::text::connected::{render_links, Link};
    let dir = temp_dir("notes");
    let h = harness(&dir, 0, Default::default(), None);
    let id = import(&h.app, "Pick it up and put it on the table.");
    wait_for("ready", || status(&h.app, &id) == AudioStatus::Ready);
    let s = &h.app.timeline(&id).unwrap().sentences[0];
    assert_eq!(render_links(s), "Pick‿it‿up‿and put‿it‿on the table.");
    assert_eq!(s.words[0].notes.link, Some(Link::ConsonantVowel));
    assert_eq!(s.words[3].notes.weak_form.as_deref(), Some("ən"));
    assert!(!s.words[0].notes.ipa.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
