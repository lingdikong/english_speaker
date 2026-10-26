# Opus-MT 英 → 中机器翻译模型

“极速翻译”（Fast）底座：随安装包分发，App 运行时直接加载，**完全离线**。用 `scripts/fetch-models.sh` 重新下载（会校验 SHA-256）。

| 文件 | 说明 | 来源 |
|---|---|---|
| `encoder_model.onnx` | 编码器，int8 量化（`encoder_model_quantized.onnx`，约 53MB） | [Xenova/opus-mt-en-zh](https://huggingface.co/Xenova/opus-mt-en-zh) @ `046f55a` |
| `decoder_model_merged.onnx` | 带 KV 缓存的合并解码器，int8 量化（`decoder_model_merged_quantized.onnx`，约 60MB） | 同上 |
| `tokenizer.json` | SentencePiece Unigram 词表与分数（共享词表 65001） | 同上 |

SHA-256：
- `encoder_model.onnx`：`d3b7912bf6a9bd27e4c074c2df91d4ff3d5b4bc5f7f6c8d7cc9c805c98fbafee`
- `decoder_model_merged.onnx`：`023be4f841f4c47cd65fffcbaa81c0d99d7f7e0138f7ba0e03fa220a4e688aff`

推理实现见 `crates/es-translate/src/mt.rs`（贪心解码 + KV 缓存；Apple Silicon 上每句 70–170ms）。

## 许可

- 模型权重：[Helsinki-NLP/opus-mt-en-zh](https://huggingface.co/Helsinki-NLP/opus-mt-en-zh)，**Apache-2.0**，作者 Language Technology Research Group at the University of Helsinki（[OPUS-MT](https://github.com/Helsinki-NLP/Opus-MT)，Tiedemann & Thottingal, 2020），训练数据来自 [OPUS](https://opus.nlpl.eu)。
- ONNX 导出与量化：Xenova（Joshua Lochner），Apache-2.0。
- Apache-2.0 全文见同目录 `LICENSE`。
