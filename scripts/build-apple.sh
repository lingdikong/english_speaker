#!/usr/bin/env bash
# 构建 iOS / iOS 模拟器 / macOS 静态库，生成 Swift 绑定，打包为 XCFramework。
# 产物：platforms/apple/SpeakerCore/（Swift Package，可直接被 Xcode 工程引用）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PKG="$ROOT/platforms/apple/SpeakerCore"
WORK="$ROOT/target/apple"
PROFILE="${PROFILE:-release}"
PROFILE_DIR="$([ "$PROFILE" = dev ] && echo debug || echo "$PROFILE")"
TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-darwin)

cd "$ROOT"
rustup target add "${TARGETS[@]}" >/dev/null
for t in "${TARGETS[@]}"; do
  cargo build -p es-ffi --profile "$PROFILE" --target "$t"
done

rm -rf "$WORK" && mkdir -p "$WORK/headers"
# 绑定与目标平台无关；release 产物会 strip，从未 strip 的主机构建生成
cargo build -q -p es-ffi
cargo run -q -p es-ffi --features bindgen --bin uniffi-bindgen -- generate \
  --library "target/debug/libes_ffi.dylib" \
  --language swift --out-dir "$WORK/bindings"
cp "$WORK/bindings/SpeakerCoreFFI.h" "$WORK/headers/"
cp "$WORK/bindings/SpeakerCoreFFI.modulemap" "$WORK/headers/module.modulemap"
mkdir -p "$PKG/Sources/SpeakerCore"
cp "$WORK/bindings/SpeakerCore.swift" "$PKG/Sources/SpeakerCore/SpeakerCore.swift"

rm -rf "$PKG/SpeakerCoreFFI.xcframework"
ARGS=()
for t in "${TARGETS[@]}"; do
  ARGS+=(-library "target/$t/$PROFILE_DIR/libes_ffi.a" -headers "$WORK/headers")
done
xcodebuild -create-xcframework "${ARGS[@]}" -output "$PKG/SpeakerCoreFFI.xcframework"
echo "✅ $PKG"
