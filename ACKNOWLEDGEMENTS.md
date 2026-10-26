# 致谢 / Acknowledgements

English Speaker 站在许多开源项目的肩膀上。感谢以下项目的作者与贡献者。
English Speaker is built on the work of many open-source projects. Thank you to their authors and contributors.

## 模型与语音数据 / Models & speech data

| 项目 | 用途 | 许可 |
|---|---|---|
| [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M)（hexgrad，训练者 @rzvzn） | 端侧 TTS 模型权重与音色 | Apache-2.0 |
| [Kokoro-82M-v1.0-ONNX](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX)（onnx-community） | ONNX 导出与 int8 量化版本、音色向量、tokenizer | Apache-2.0 |
| [StyleTTS 2](https://github.com/yl4579/StyleTTS2)（Li et al., @yl4579） | Kokoro 的模型架构 | MIT |
| [misaki](https://github.com/hexgrad/misaki)（hexgrad） | 英文 G2P 发音词典（美式 / 英式 gold & silver） | Apache-2.0 |
| [Opus-MT en-zh](https://huggingface.co/Helsinki-NLP/opus-mt-en-zh)（Helsinki-NLP）/ [ONNX 版](https://huggingface.co/Xenova/opus-mt-en-zh)（Xenova） | 极速翻译：英 → 中机器翻译 | Apache-2.0 |
| [ECDICT](https://github.com/skywind3000/ECDICT)（skywind3000） | 离线查词：英汉词典、考试标签、词形变化 | MIT |
| [Qwen2.5-0.5B-Instruct](https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF)（阿里云通义千问团队） | 智能翻译、点词举一反三、句子解析（按需下载） | Apache-2.0 |

Kokoro v1.0 的训练数据中包含以下 CC BY 音频（按其许可要求署名）：
- [Koniwa](https://github.com/koniwa/koniwa) `tnc` — [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/)
- [SIWIS](https://datashare.ed.ac.uk/handle/10283/2353) — [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)

## 运行时与库 / Runtime & libraries

| 项目 | 用途 | 许可 |
|---|---|---|
| [ONNX Runtime](https://github.com/microsoft/onnxruntime) | 模型推理 | MIT |
| [ort](https://github.com/pykeio/ort) | ONNX Runtime 的 Rust 绑定 | MIT / Apache-2.0 |
| [llama.cpp](https://github.com/ggml-org/llama.cpp) / [llama-cpp-2](https://github.com/utilityai/llama-cpp-rs) | 本地大模型推理（Metal / CPU）与 GBNF 语法采样 | MIT / MIT or Apache-2.0 |
| [ureq](https://github.com/algesten/ureq)、[rustls](https://github.com/rustls/rustls)、[RustCrypto sha2](https://github.com/RustCrypto/hashes) | 模型下载（HTTPS、断点续传、SHA-256 校验） | MIT / Apache-2.0 |
| [SQLite](https://sqlite.org) / [rusqlite](https://github.com/rusqlite/rusqlite) | 缓存池与文章库 | Public Domain / MIT |
| [UniFFI](https://github.com/mozilla/uniffi-rs)（Mozilla） | Rust ↔ Swift / Kotlin 绑定 | MPL-2.0 |
| [cpal](https://github.com/RustAudio/cpal) | 桌面端音频输出 | Apache-2.0 |
| [Chromium Embedded Framework](https://bitbucket.org/chromiumembedded/cef) / [cef-rs](https://github.com/tauri-apps/cef-rs) | 桌面端界面宿主 | BSD-3-Clause / MIT / Apache-2.0 |
| [serde](https://serde.rs)、thiserror、uuid、log、crossbeam、objc2 等 Rust crate | 基础设施 | MIT / Apache-2.0 |
| [Svelte](https://svelte.dev)、[Vite](https://vitejs.dev)、TypeScript | 桌面端 Web 界面 | MIT / Apache-2.0 |
| Jetpack Compose、AndroidX、Media3、Kotlin Coroutines | Android 界面与播放服务 | Apache-2.0 |
| [Haze](https://github.com/chrisbanes/haze)（Chris Banes） | Android 玻璃模糊效果 | Apache-2.0 |
| SwiftUI、AVFoundation、MediaPlayer | iOS 界面与音频 | Apple SDK |

完整的 Rust 依赖清单见 `Cargo.lock`，Web 依赖见 `ui/web/package-lock.json`，Android 依赖见 `platforms/android/gradle/libs.versions.toml`。

## 字体 / Fonts

| 字体 | 用途 | 许可 |
|---|---|---|
| [Space Grotesk](https://github.com/floriankarsten/space-grotesk)（Florian Karsten） | 标题与品牌 | SIL OFL 1.1 |
| [Manrope](https://github.com/sharanda/manrope)（Mikhail Sharanda） | 界面与原文 | SIL OFL 1.1 |
| [Noto Sans](https://github.com/notofonts/latin-greek-cyrillic)（Google） | IPA 音标 | SIL OFL 1.1 |

字体随应用离线打包，许可文本随字体文件一起分发。

## 说明 / Notes

- 本项目的界面为自有设计（见 `docs/design/visual-v2.html`、`docs/UI_SPEC.md`），与任何第三方产品无关。
- “Apple Music”“SwiftUI”等名称为其各自所有者的商标；本项目与 Apple、Google、Microsoft 均无隶属关系。
