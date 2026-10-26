# English Speaker 架构

把英文文章离线转换为语音播客。五个平台：Windows、macOS、Linux、iOS、Android。

**原则：业务逻辑只写一次（Rust 核心层），平台层只做"界面 + 放声音 + 系统集成"。**

## 1. 分层

```
┌──────────────────────────────────────────────────────────────────────┐
│ 平台壳 (UI + 系统集成)                                                │
│  iOS / macOS(移动风格)   Android            Windows / macOS / Linux   │
│  SwiftUI                 Jetpack Compose    Chromium(CEF) + Web UI    │
│  AVAudioEngine sink      AudioTrack sink    cpal sink (Rust)          │
│  MPNowPlayingInfoCenter  MediaSession       SMTC / NowPlaying / MPRIS │
└───────▲──────────────────────▲─────────────────────▲─────────────────┘
        │ Swift 绑定           │ Kotlin 绑定          │ JSON 桥
        │ (UniFFI, 自动生成)   │ (UniFFI, 自动生成)   │ (serde)
┌───────┴──────────────────────┴─────┐      ┌────────┴──────────┐
│ es-ffi  类型翻译 + 转发，无业务逻辑  │      │ es-desktop 宿主   │
└───────────────────▲────────────────┘      └────────▲──────────┘
                    └──────────────┬─────────────────┘
                     ┌─────────────┴─────────────┐
                     │ es-runtime  组装根          │  选择具体实现并注入
                     └─────────────▲─────────────┘
       ┌──────────────┬────────────┼──────────────┬──────────────┐
┌──────┴──────┐ ┌─────┴──────┐ ┌───┴─────────┐ ┌──┴──────────┐   │
│es-tts-kokoro│ │ es-storage │ │  es-audio   │ │ (平台 sink) │   │
│ TtsEngine   │ │ Storage    │ │ AudioCodec  │ │ AudioSink   │   │
│ ort + Kokoro│ │ SQLite     │ │ PCM → Opus  │ │             │   │
└──────▲──────┘ └─────▲──────┘ └───▲─────────┘ └──▲──────────┘   │
       └──────────────┴────────────┴──────────────┴──────────────┘
                     实现 ports 中的 trait
                     ┌───────────────────────────┐
                     │ es-core  （零平台依赖）     │
                     │  model   领域模型           │
                     │  text    清洗/分句/规范化   │
                     │  synth   合成队列与编排     │
                     │  player  播放引擎/时间轴    │
                     │  app     AppCore 门面       │
                     │  ports   依赖抽象 (trait)   │
                     └───────────────────────────┘
```

依赖方向严格向下：`es-core` 不依赖任何其他 crate、不含任何 `cfg(target_os)`。

| Crate | 职责 | 平台相关性 |
|---|---|---|
| `es-core` | 领域模型、文本流水线、合成编排、播放引擎、`AppCore` 门面 | 无（纯 Rust） |
| `es-tts-kokoro` | Kokoro-82M 推理（`ort`）+ 无 GPL 的英文 G2P | 仅执行后端（CoreML / NNAPI / DirectML，可选） |
| `es-storage` | SQLite（bundled，迁移脚本） | 无 |
| `es-audio` | 按块编解码（PCM s16；Opus 计划中） | 无 |
| `es-translate` | 翻译与查词：Opus-MT（`ort`）、ECDICT（SQLite）、Qwen（llama.cpp，`llm` 特性），见 [TRANSLATION.md](TRANSLATION.md) | Metal（Apple）/ CPU |
| `es-onnx` | 全进程共用的 ONNX Runtime 环境与平台加速后端（Kokoro 与 Opus-MT 共用） | 执行后端 |
| `es-runtime` | 组装根：按配置选择实现，FFI 和桌面共用 | 无 |
| `es-ffi` | UniFFI 导出 `SpeakerCore`，生成 Swift / Kotlin | 产物：`.a` / `.so` |
| `es-desktop` | 桌面宿主：JSON 桥、cpal 音频输出、开发 CLI、（CEF 窗口） | 桌面三平台 |

## 2. 核心与平台的边界

平台**只**需要实现两个接口：

```rust
trait AudioSink {            // 把 PCM 放出声音
    fn open(&self, format) -> Result<()>;
    fn write(&self, samples: &[f32]) -> Result<()>;   // 缓冲 > ~200ms 时阻塞（背压）
    fn pause(&self); fn resume(&self); fn flush(&self);
    fn played_frames(&self) -> u64;                    // 设备实际播放帧数 → 进度的唯一来源
    fn set_rate(&self, rate: f32) -> bool;             // 变速不变调，平台原生实现
    fn close(&self);
}
trait EventListener { fn on_event(&self, e: CoreEvent); }  // 可能在后台线程回调
```

再调用 `AppCore`（移动端叫 `SpeakerCore`）的方法：导入、列表、合成、播放、跳句、变速。

**为什么解码和播放逻辑放在核心层，而不是交给 AVPlayer / ExoPlayer？**
- 进度、断点续播、逐词高亮、跳句、AB 循环等功能，全部基于 `played_frames` 加时间轴计算，五个平台行为完全一致；
- 音频格式由核心层决定（PCM → Opus），不受各平台解码器支持范围的限制；
- 平台 sink 很薄：iOS 用 `AVAudioEngine` + `AVAudioPlayerNode.scheduleBuffer`，
  Android 用阻塞模式的 `AudioTrack.write`，桌面用 cpal。后台播放和锁屏控件仍由平台原生提供。

## 3. 数据流

```
导入(粘贴/文件/系统分享)
  → text::ingest    去 Markdown、分段
  → text::segment   分句（缩写、引号、小数），超长句按标点切分 ≤300 字符
  → storage         articles + sentences（全部句子文本先入库：原文立即可见）
  → synth 队列      导入即入队，开始后台转换
      每句：normalize（$3.5M → three point five million dollars）→ TtsEngine
            → 句前停顿 + 语音编码为一个块 → 追加写入缓存文件
            → 一个事务提交：块位置 + 逐词时间轴 + 缓存记录累计（句数/帧数/字节数）
            → 通知播放器 DataAvailable，UI 收到 SynthesisProgress 点亮该句
  → player          只读已提交的块 → 解码 → AudioSink
                    追上合成进度 → Buffering，有新块后自动继续（边听边转）
  → UI              PlayerPosition{position, buffered, sentence, word} 驱动进度条与“歌词式”高亮
```

原文与朗读文本分离：高亮显示的是原文 `$5`，朗读的是 `five dollars`。

## 4. 边听边转与缓存池

### 4.1 存储布局

```
<data_dir>/
  library.db                          SQLite：所有索引
  audio/<article_id>-<rand>.pcm       每篇一个只追加文件，内容是逐句的块
```

| 表 | 作用 |
|---|---|
| `articles` | 原文、音色偏好、`pinned`（保留下载）、句数、估算时长 |
| `sentences` | 每句文本；合成后写入 `byte_offset/byte_len/start_frame/frames` 与逐词时间轴 |
| `cache_entries` | 缓存池：每篇一条，`state`、`ready_sentences`、`byte_size`、`last_access_ms`（LRU 依据）、`file_name` |
| `progress` / `settings` | 播放进度；缓存上限等设置 |

**块（chunk）= 一句话**，每块独立编码、独立解码，类似 HLS 分片：
- **边写边读**：播放器只读 SQL 中已提交的块，写入方只追加，不需要锁文件；
- **断点续转**：任务中断（App 被杀、被抢占、用户暂停）后，从第一个未提交的句子继续；
  恢复时先把文件截断到最后一个已提交块，丢弃写了一半的数据；
- **换编码器**：Opus 只需实现 `AudioCodec::encode/decode` 一个块，核心层不改。

### 4.2 状态

```
NotCached ──导入/播放/钉住──▶ Queued ──▶ Synthesizing ──全部完成──▶ Ready
   ▲                           ▲  │被抢占        │取消/中断
   │ LRU 淘汰 / 移除下载        │  ▼             ▼
   └──────────────────────── Queued ◀──继续── Partial          Failed
```

### 4.3 调度

- 导入即入队（FIFO）；**点播**未缓存完的文章时插到队首，当前任务在句边界让出，之后从断点续转；
- 播放位置可以拖到尚未合成的地方：进入 Buffering，合成追上后自动开始；
- 合成完成前，时长 = 已合成的真实时长 + 未合成部分按文本估算（`duration_final = false`）。

### 4.4 缓存池

- 上限默认 1 GiB，`set_cache_limit` 修改后存入 `settings`；
- 每提交一块和每次修改上限时检查：超过上限就按 `last_access_ms` 从旧到新淘汰；
- **不淘汰**：钉住的（"保留下载"）、正在播放的、正在合成的；
- 淘汰只删除音频文件和块索引，文章和原文保留在资料库；再次播放时重新流式合成；
- `set_pinned(true)` 会确保文章完整合成（相当于 Apple Music 的"下载"）；还有 `remove_download` 和 `clear_cache`。
- 换音色时，旧任务可能还没停下来就提交块。所以提交时校验 `file_name`（每次重建都会生成新的随机后缀），旧任务的写入会被拒绝。

## 5. TTS 引擎（Kokoro-82M）

| 项 | 选择 |
|---|---|
| 模型 | Kokoro-82M v1.0 ONNX（onnx-community 导出，Apache-2.0），默认 int8 量化 `model_quantized.onnx`（88MB）；`VARIANT=fp16/q8f16/fp32` 可切换 |
| 推理 | `ort` 2.0（ONNX Runtime 1.31，静态链接 pyke 预编译包）；默认 CPU、4 线程、图优化 Level1 |
| 音色 | 6 个精选英文音色（美式 af_heart/af_bella/am_michael/am_fenrir，英式 bf_emma/bm_george），每个 510KB |
| G2P | misaki 美式/英式词典（Apache-2.0，约 18 万词条，直接是 Kokoro 音素）→ 词形变化 → 连字符/数字/缩写 → 复合词 → 拼写规则兜底；**不含 espeak-ng（GPL）** |
| 逐词时间 | 该 ONNX 导出只输出波形，按每个词的音素数在句内分配 |
| 连读 | 见下方说明 |
| 加载 | 构造约 2ms 立即返回（音色列表可用），会话在后台线程加载约 1–3s，首次合成时等待 |

**连读（connected speech）分三层处理：**

1. **声学层（模型负责）**：Kokoro 按整句一次合成，不是逐词拼接。辅音接元音（pick‿it‿up）、同化（did you → /dɪʤu/）、
   省音（next day）、跨词闪音（get‿it）这些衔接，都由模型从训练用的真实语音里学到。
   前提是**整句送入模型**：核心层只在句间、段间插入停顿；超长句只在逗号、分号处切分，因为那里本来就会停顿。
2. **音素层（G2P 负责，必须与训练时一致）**：
   - 虚词用词典里不带重音的形式（and → ænd，of → ʌv），模型会自动弱读；
   - the / a / an / am / to / in / I 按 misaki 规则选择形式。是否“后接元音”按下一个词的**音素**判断：
     the hour → ði，the university → ðə，an hour → ɐn，to eat → tʊ，to go → tə，句末 to → tu；
     所以整句从右往左处理；
   - 美式闪音写作 `T`、喉塞音写作 `t`（Kokoro v1.0 的训练约定，词典原文是 ɾ / ʔ）。
3. **不做的**：wanna、gonna、gotta、didja 这类口语缩读。朗读文章时不应出现，强行改写也会偏离训练分布。

**跟读标注**（`es-core/src/text/connected.rs`，已实现）：合成每句时，根据引擎返回的逐词音素，
按规则为每个词标注，结果写入时间轴（`WordSpan.notes`），随缓存持久化，FFI 与 JSON 桥都可用：

| 字段 | 含义 | 例 |
|---|---|---|
| `link` | 与下一个词的连读：辅音接元音 / 加 /j/ / 加 /w/ / 英式连接 r / 相同辅音合并 / 美式跨词闪音 / 同化 | pick‿it‿up、see‿it、go‿on、far‿away、big‿game、get‿it、did‿you |
| `weak_form` | 虚词弱读（IPA）；只在句中不重读时给出，句末虚词用强读 | and /ən/、of /əv/、to /tə/ |
| `flap` | 词内美式闪音 | water /wˈɔɾəɹ/ |
| `elided` | 辅音丛结尾的 t/d 在辅音前省略 | next day |
| `ipa` | 词典读音（标准 IPA） | table /tˈeɪbəl/ |

逗号、句号等停顿处不连读。英式音色没有闪音，但有连接 r。
标注是教学用的规则分析，与实际音频中模型自然产生的连读大体一致，但不保证逐处吻合。
UI 用结构化字段渲染：两词之间画 ‿，弱读词淡化，点按某个词显示 IPA 和说明，并结合逐词高亮跟读。
`connected::render_links` 提供纯文本版本，供 CLI 和测试使用。

模型文件：`scripts/fetch-models.sh` 下载到 `models/kokoro/`（共约 112MB，校验 SHA-256，版本锁定），随安装包分发。

**实测 RTF（合成耗时 / 音频时长，越小越快；int8，CPU 4 线程）：**

| 环境 | 会话加载 | RTF |
|---|---|---|
| macOS（Apple Silicon，原生） | 2.6s | 0.09–0.11 |
| macOS + CoreML EP | 2.9s | 0.09–0.12（无收益，默认关闭） |
| iOS 模拟器（宿主 CPU） | — | 0.08 |
| Android arm64 模拟器（宿主 CPU 虚拟化，4 核） | 1.1s | 0.11 |

模拟器跑在 Mac 的 CPU 上，只能证明代码路径可用，不代表手机性能；真机数据待测。
RTF 0.1 意味着合成速度是播放速度的 10 倍，边听边转时首句约 0.3–0.8s 就绪（会话冷启动另加 1–3s）。

**平台构建要点：**
- Apple：ORT 静态库要求 macOS ≥ 13.4 / iOS ≥ 16（`.cargo/config.toml`），并需链接 clang 运行时（`es-tts-kokoro/build.rs`）；
  Swift Package 需链接 `CoreML`、`Foundation`、`c++`。
- Android：仅 `arm64-v8a` 与 `x86_64`（ORT 无 armeabi-v7a 预编译包）；`libc++_shared.so` 随 APK 分发；
  链接 16KB 页对齐（Android 15+ / Google Play 要求）。
- macOS x86_64（Intel Mac）没有 ORT 预编译包，如需支持须自行编译 ORT。
- release 配置会 strip，过程宏须排除（`[profile.release.build-override]`）；UniFFI 绑定从未 strip 的主机构建生成。

## 6. 线程模型

| 线程 | 内容 |
|---|---|
| 调用方线程（UI） | `AppCore` 方法都是非阻塞的：要么快速查库，要么向后台线程发命令 |
| `es-synth` | 优先级队列，同一时间合成一篇，逐句提交；支持抢占、取消、换音色；重启后从断点续转 |
| `es-player` | 播放状态机（含 Buffering），按块读取并推送 PCM，约 100ms 发一次位置事件，约 5s 保存一次进度 |
| `es-audio-out`（桌面） | 持有 cpal Stream（部分平台上 Stream 不是 `Send`） |

事件回调来自后台线程。Swift 端用 `DispatchQueue.main.async` 切回主线程，Kotlin 端用 `Dispatchers.Main`，Web 端由宿主投递。

## 7. 离线与资源

- 模型随安装包分发：iOS 放 Bundle，Android 用 install-time asset pack，桌面放 `Resources/models`；
  路径通过 `RuntimeConfig.model_dir` 传入。
- 数据库里只存音频的**相对文件名**，因为 iOS 沙盒路径会随 App 更新变化。
- 没有模型文件时，`es-runtime` 退回 `MockTtsEngine`（正弦音）；单元测试和 CI 的大部分任务不需要下载模型。

## 8. 构建与产物

| 平台 | 命令 | 产物 |
|---|---|---|
| iOS + macOS | `scripts/build-apple.sh` | `platforms/apple/SpeakerCore`（Swift Package + XCFramework） |
| Android | `scripts/build-android.sh` | `platforms/android/core/src/main/{jniLibs,kotlin}` |
| 桌面 | `cargo build -p es-desktop --release` | `es-desktop` 可执行文件 |
| 全部 | `.github/workflows/ci.yml` | Win/Mac/Linux 原生测试 + Apple + Android |

开发调试：

```bash
./scripts/fetch-models.sh                             # 下载 Kokoro 模型（约 112MB）
cargo test --workspace                                # 有模型时会同时跑真实引擎测试
ES_MODEL_DIR=models cargo run --release -p es-desktop -- speak article.txt   # 真实语音，边听边转
cargo run --release -p es-tts-kokoro --example kokoro_bench -- models out    # RTF 基准 + 试听 WAV
cargo run -p es-desktop -- bridge --mute              # stdin 输入 JSON 命令，用于 UI 联调
(cd platforms/apple/SpeakerCore && swift test)        # Swift 经 FFI 调用真实核心
```

## 9. 当前状态与下一步

已完成并验证：
- 核心层全链路：导入即转换 → 边听边转（含缓冲）→ 断点续转（含崩溃恢复与截断）→ LRU 缓存池 / 钉住 / 清理 → 换音色 → 删除；
- iOS / 模拟器 / macOS 的 XCFramework，以及 Swift 冒烟测试（Rust ↔ Swift 双向回调）；
- Android arm64-v8a / armeabi-v7a / x86_64 的 `.so` 和 Kotlin 绑定；
- 桌面 JSON 桥和 CLI（macOS 实测）。

下一步：
1. **真机性能**：在中端 Android、iPhone 上测 RTF、内存、发热；必要时换 fp16 / 调线程数；
2. **G2P 质量**：未登录词（人名、新词）目前靠拼写规则，可接入小型神经网络 G2P（如 misaki 的 BART 兜底）；
   多音词（read/record）暂取默认读音，需要词性标注才能区分；
3. **Opus 编码**（`es-audio`）：音频体积降到约 1/10；
4. **桌面变速**：在 Rust 侧实现保持音调的 WSOLA 时间伸缩（移动端用平台原生能力）；
5. **平台壳**：已完成 v2 自有视觉（见 `docs/UI_SPEC.md`，截图 `docs/screenshots/v2/`）；后续在真机上验证性能与流畅度；
6. **导入**：PDF / EPUB / HTML 正文抽取，iOS Share Extension、Android `ACTION_SEND`；
7. 跳到远处未合成的位置时，让合成从该处开始（目前是顺序合成，需要等合成追上来）。
