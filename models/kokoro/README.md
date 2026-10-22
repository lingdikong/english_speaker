# Kokoro-82M 模型文件

本目录随仓库分发，App 运行时直接从这里加载，**完全离线**。也可以用 `scripts/fetch-models.sh` 重新下载（会校验 SHA-256）。

| 文件 | 说明 | 来源 |
|---|---|---|
| `model.onnx` | Kokoro-82M v1.0，int8 量化（`model_quantized.onnx`，约 88MB） | [onnx-community/Kokoro-82M-v1.0-ONNX](https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX) @ `1939ad2` |
| `tokenizer.json` | 音素 → token 映射 | 同上 |
| `voices/*.bin` | 音色向量：af_heart、af_bella、am_michael、am_fenrir（美音），bf_emma、bm_george（英音） | 同上 |
| `g2p/{us,gb}_{gold,silver}.json` | 英文发音词典 | [hexgrad/misaki](https://github.com/hexgrad/misaki) @ `fba1236` |
| `VARIANT` | 当前精度（`quantized`） | — |

SHA-256（`model.onnx`）：`fbae9257e1e05ffc727e951ef9b9c98418e6d79f1c9b6b13bd59f5c9028a1478`

## 许可

- Kokoro-82M 权重与音色：**Apache-2.0**，作者 [hexgrad](https://huggingface.co/hexgrad/Kokoro-82M)（训练者 @rzvzn），架构来自 [StyleTTS 2](https://github.com/yl4579/StyleTTS2)（MIT）。
- ONNX 导出：onnx-community，Apache-2.0。
- misaki 词典：Apache-2.0，作者 hexgrad。
- Apache-2.0 全文见同目录 `LICENSE`。

Kokoro v1.0 训练数据含 CC BY 音频：[Koniwa](https://github.com/koniwa/koniwa) `tnc`（CC BY 3.0）、[SIWIS](https://datashare.ed.ac.uk/handle/10283/2353)（CC BY 4.0）。
