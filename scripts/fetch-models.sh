#!/usr/bin/env bash
# 下载随安装包分发的离线模型（见 docs/ARCHITECTURE.md、docs/TRANSLATION.md）：
#   models/kokoro/         Kokoro-82M（ONNX，Apache-2.0）、精选英文音色、misaki 英文 G2P 词典（Apache-2.0）
#   models/opus-mt-en-zh/  Opus-MT 英→中机器翻译（ONNX int8，Apache-2.0）
#   models/dict/           ECDICT 核心词库（MIT），由 scripts/build-dict.py 从原始 CSV 生成
# 智能翻译模型（Qwen，约 430MB）不随包分发，由 App 按需下载到数据目录。
#
#   VARIANT=quantized|q8f16|fp16|fp32  选择模型精度（默认 quantized，约 88MB）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/models/kokoro"
HF="https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/1939ad2a8e416c0acfeecc08a694d14ef25f2231"
MISAKI="https://raw.githubusercontent.com/hexgrad/misaki/fba1236595f2d2bf21d414ba6e57d25256afada3/misaki/data"
OPUS="https://huggingface.co/Xenova/opus-mt-en-zh/resolve/046f55aec303cdee3e0318604406d4df20f1e8ea"
ECDICT="https://raw.githubusercontent.com/skywind3000/ECDICT/ab22ad36302cafc6349a050e2b60894855208055"
VARIANT="${VARIANT:-quantized}"
VOICES=(af_heart af_bella am_michael am_fenrir bf_emma bm_george)

case "$VARIANT" in
  quantized) MODEL=model_quantized.onnx; SHA=fbae9257e1e05ffc727e951ef9b9c98418e6d79f1c9b6b13bd59f5c9028a1478 ;;
  q8f16)     MODEL=model_q8f16.onnx;     SHA=04c658aec1b6008857c2ad10f8c589d4180d0ec427e7e6118ceb487e215c3cd0 ;;
  fp16)      MODEL=model_fp16.onnx;      SHA=ba4527a874b42b21e35f468c10d326fdff3c7fc8cac1f85e9eb6c0dfc35c334a ;;
  fp32)      MODEL=model.onnx;           SHA="" ;;
  *) echo "unknown VARIANT=$VARIANT" >&2; exit 2 ;;
esac

fetch() { # url dest [sha256]
  if [ -s "$2" ] && { [ -z "${3:-}" ] || echo "$3  $2" | shasum -a 256 -c --status; }; then return; fi
  echo "↓ $(basename "$2")"
  curl -fsSL --retry 3 -o "$2.part" "$1"
  if [ -n "${3:-}" ]; then echo "$3  $2.part" | shasum -a 256 -c --status || { echo "sha256 mismatch: $2" >&2; exit 1; }; fi
  mv "$2.part" "$2"
}

mkdir -p "$OUT/voices" "$OUT/g2p"
fetch "$HF/onnx/$MODEL" "$OUT/model.onnx" "$SHA"
fetch "$HF/tokenizer.json" "$OUT/tokenizer.json"
for v in "${VOICES[@]}"; do fetch "$HF/voices/$v.bin" "$OUT/voices/$v.bin"; done
for d in us_gold us_silver gb_gold gb_silver; do fetch "$MISAKI/$d.json" "$OUT/g2p/$d.json"; done
echo "$VARIANT" > "$OUT/VARIANT"

MT="$ROOT/models/opus-mt-en-zh"
mkdir -p "$MT"
fetch "$OPUS/onnx/encoder_model_quantized.onnx" "$MT/encoder_model.onnx" d3b7912bf6a9bd27e4c074c2df91d4ff3d5b4bc5f7f6c8d7cc9c805c98fbafee
fetch "$OPUS/onnx/decoder_model_merged_quantized.onnx" "$MT/decoder_model_merged.onnx" 023be4f841f4c47cd65fffcbaa81c0d99d7f7e0138f7ba0e03fa220a4e688aff
fetch "$OPUS/tokenizer.json" "$MT/tokenizer.json" d0c7da27056e8f42adce9e76d8e792e5daa64e15f5acd2e7aabf0121877dd4c1

DICT="$ROOT/models/dict"
if [ ! -s "$DICT/ecdict.db" ]; then
  TMP="$(mktemp -d)"
  fetch "$ECDICT/ecdict.csv" "$TMP/ecdict.csv" d1ecbdf9ee2b920afb6f430ca6b6bb9f559588cf0dff86e04bd255f9390843cc
  mkdir -p "$DICT"
  python3 -I "$ROOT/scripts/build-dict.py" "$TMP/ecdict.csv" "$DICT/ecdict.db"
  fetch "$ECDICT/LICENSE" "$DICT/LICENSE"
  rm -rf "$TMP"
fi
du -sh "$OUT" "$MT" "$DICT"
