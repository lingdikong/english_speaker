# 翻译与查词（双模架构）

完全离线、开源可商用的英→中翻译与查词。**核心逻辑全部在 Rust 核心层**（`es-core::translate` + `es-translate`），
五个平台共用同一套接口与行为，平台层只负责展示（UI 沿用各端原有组件与写法）。

## 1. 两种模式

| | Fast：极速翻译（默认） | Smart：智能翻译（设置中开启） |
|---|---|---|
| 整句翻译 | Opus-MT en→zh（ONNX int8，ORT） | Qwen2.5-0.5B-Instruct（GGUF Q4_0，llama.cpp），以 Fast 译文为参考精修 |
| 查词 | ECDICT 核心词库（SQLite），变形还原 | 同左 + **举一反三**：语境义、常用搭配、地道例句 |
| 句子解析 | — | 意译、句子结构、1–3 个语法点 |
| 术语表 | 占位符保护（`XQA` → 固定译法） | 写进提示词 |
| 分发 | 随安装包（MT 108MB + 词库 7MB） | 按需下载 409MB（ModelScope → HuggingFace 回退） |
| 内存 | ≈ 150MB（两个 ORT 会话） | 再加 ≈ 500MB（关闭智能翻译即释放） |
| 速度（M 系列 Mac 实测） | 70–170ms / 句；查词 10–40µs | 精翻 80–120ms / 句（Metal）；举一反三 0.6–0.8s；句子解析 1–1.6s |
| 协议 | Apache-2.0 / MIT | Apache-2.0（llama.cpp MIT） |

智能模式打开但模型未下载、或加载失败时，自动退回 Fast；平台层不需要分支。

## 2. 分层

```
平台 UI（SwiftUI / Compose / Svelte）——只展示，调用同一组方法、处理同一组事件
        │  UniFFI（SpeakerCore）· JSON 桥（es-desktop bridge）
es-core::AppCore           translator_status / translations / prepare_translations / lookup_word
                           word_insight / sentence_analysis / set_smart_translation / download_llm …
es-core::translate         调度线程 es-translate、模式切换、术语表、提示词 + GBNF、结果解析与缓存
        │  ports: Translator · Dictionary · LlmProvider · LanguageModel
es-translate               mt.rs（Opus-MT）· dict.rs（ECDICT）· llm.rs（Qwen + 下载器，feature = "llm"）
es-onnx                    全进程共用的 ORT 环境（Kokoro 与 Opus-MT 共用，ORT 只允许建一次）
es-storage                 translations / ai_notes / glossary / vocabulary 四张表（迁移 v3）
```

## 3. 调度（es-translate 线程）

- **导入即预翻**：每篇入队，Fast 逐句翻译写入 `translations(article, idx, engine)`；启动时继续上次没翻完的。
- **正在看的文章优先**（`play_article` / `prepare_translations` → focus）：插到队首；智能模式下再逐句 Smart 精翻。
  其它文章只做 Fast，避免大模型在后台耗电。
- **插队任务**：点词举一反三、句子解析在两句之间优先执行；结果写入 `ai_notes` 缓存，下次直接返回。
- **术语表变化**：作废原文含该术语的句子的译文（与整句解析），重新翻译，并发 `TranslationsChanged`。
- 展示时每句取一条：Smart 优先，没有时用 Fast（`translations()`）。

事件：`TranslationProgress`（逐句）· `TranslationsChanged` · `WordInsightReady` · `SentenceAnalysisReady` ·
`TranslatorChanged`（模式 / 下载进度约 4 次每秒 / 加载）· `VocabularyChanged`。

## 4. 小模型提示词（实测结论）

0.5B 模型能力有限，`translate/prompt.rs` 的做法都来自 `examples/translate_bench.rs` 的对比：

1. **少样本胜过说明**：每类任务带 1–2 轮示例对话（`LlmRequest::shots`）。不加示例时，点词的词性会写成一段说明、释义照抄词典。
2. **不给上文**：把上一句放进提示词时，5 句里有 3 句模型去翻了上文，所以整句翻译只看本句。
3. **机器译文作参考**：不给参考时译文更顺，但会凭空加内容（“放在冰箱里”）；给了参考，模型主要修标点、术语与语序，稳定得多。
   代价是 Smart 整句译文只比 Fast 略好，智能模式的主要价值在术语、举一反三与句子解析。
4. **GBNF 约束结构**：词性限定为枚举、释义限长；解析时再过滤（搭配必须含该词、语法点必须是原句片段）。
   失败时 UI 退回词典释义。

需要更好的整句质量时，可换 Qwen2.5-1.5B（≈1GB，内存约 1.3GB）：只需在 `llm.rs` 增加一个 `ModelSpec`。

## 5. 模型文件

| 目录 | 内容 | 获取 |
|---|---|---|
| `models/opus-mt-en-zh/` | encoder / merged decoder（int8）+ tokenizer.json | `scripts/fetch-models.sh`（SHA-256 锁定） |
| `models/dict/ecdict.db` | 5.9 万词 + 3.9 万变形 | `scripts/build-dict.py` 从 ECDICT CSV 生成 |
| `<data_dir>/models/qwen2.5-0.5b-instruct-q4_0.gguf` | 智能翻译模型 | App 内下载：Range 断点续传，完成后 SHA-256 校验，失败换源 |

分词：Opus-MT 的 SentencePiece Unigram 在 `mt.rs` 里自己做 Viterbi，不引入 `tokenizers`（避免 iOS / Android 交叉编译问题）。

## 6. 平台要点

- **Apple**：llama.cpp 用 Metal（模拟器用 CPU）；Swift Package 链接 `Metal`、`MetalKit`、`Accelerate`。
  下载前用 `NWPathMonitor.isExpensive` 判断蜂窝网络并在确认框里提示。
- **Android**：llama.cpp 与 ORT 共用 NDK `c++_shared`；`build-android.sh` 需要 cmake 与 `CXX_*`；
  清单新增 `INTERNET`、`ACCESS_NETWORK_STATE`（只用于下载模型），按 `isActiveNetworkMetered` 提示。
  没有用 NNAPI 跑大模型（Android 15 已弃用），用 CPU（ARM NEON，Q4_0 有重排加速）。
- **桌面**：模型随包内 `models/`；JSON 桥新增同名命令。

## 7. 测试

- `cargo test -p es-core`：术语匹配 / 占位保护、提示词与 JSON 解析。
- `cargo test -p es-storage`：译文作废、术语表大小写、生词本在文章删除后保留。
- `cargo test -p es-runtime`：端到端（Mock MT / 词典 / 大模型）：预翻 → 术语重翻 → 下载 → 精翻 → 举一反三 → 生词本 → 删除模型。
- `cargo test -p es-translate`：真实 Opus-MT 与词库（有模型时）。
- `cargo run --release -p es-translate --example translate_bench -- models <llm_dir>`：质量与速度对比。
- `cargo run --release -p es-translate --example llm_download -- <dir>`：下载 / 续传 / 校验。

## 8. 已知限制

- 真机性能未测（RTF、内存、发热），以上速度均为 Apple Silicon Mac 与模拟器数据。
- Smart 整句精翻质量受 0.5B 模型限制（见 §4）；举一反三的词性偶有错误。
- Opus-MT 贪心解码，未做 beam search；超长句（>400 token）截断。
- 词典只收常用词，专有名词与生僻词查不到时显示无释义。
