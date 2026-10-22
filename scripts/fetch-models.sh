#!/usr/bin/env bash
# 下载 Kokoro-82M（ONNX，Apache-2.0）、精选英文音色和 misaki 英文 G2P 词典（Apache-2.0）。
# 产物：models/kokoro/（随安装包分发的模型目录，见 docs/ARCHITECTURE.md）
#
#   VARIANT=quantized|q8f16|fp16|fp32  选择模型精度（默认 quantized，约 88MB）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/models/kokoro"
HF="https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/1939ad2a8e416c0acfeecc08a694d14ef25f2231"
MISAKI="https://raw.githubusercontent.com/hexgrad/misaki/fba1236595f2d2bf21d414ba6e57d25256afada3/misaki/data"
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
du -sh "$OUT"
