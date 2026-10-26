#!/usr/bin/env bash
# 构建 Android 各 ABI 的 .so，生成 Kotlin 绑定。
# 产物：platforms/android/core/src/main/{jniLibs,kotlin}
# 依赖：ANDROID_NDK_HOME（默认取 $ANDROID_HOME/ndk 下最新版本）；cmake（编译 llama.cpp）
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/platforms/android/core/src/main"
PROFILE="${PROFILE:-release}"
PROFILE_DIR="$([ "$PROFILE" = dev ] && echo debug || echo "$PROFILE")"
API=24
SDK="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
NDK="${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -1)}"
HOST_TAG="$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64"
BIN="$NDK/toolchains/llvm/prebuilt/$HOST_TAG/bin"

# rust target : android ABI : clang 前缀
# 不含 armeabi-v7a：ONNX Runtime 没有该 ABI 的预编译包（32 位设备已很少）
ABIS=(
  "aarch64-linux-android:arm64-v8a:aarch64-linux-android"
  "x86_64-linux-android:x86_64:x86_64-linux-android"
)

cd "$ROOT"
# llama.cpp（智能翻译）用 cmake + NDK 工具链编译，C++ 运行时与 ONNX Runtime 一样用 c++_shared
export ANDROID_NDK="$NDK" ANDROID_API_LEVEL="$API"
for entry in "${ABIS[@]}"; do
  IFS=: read -r target abi clang <<<"$entry"
  rustup target add "$target" >/dev/null
  env_target="$(echo "$target" | tr '[:lower:]-' '[:upper:]_')"
  export "CC_${target//-/_}=$BIN/${clang}${API}-clang"
  export "CXX_${target//-/_}=$BIN/${clang}${API}-clang++"
  export "AR_${target//-/_}=$BIN/llvm-ar"
  export "CARGO_TARGET_${env_target}_LINKER=$BIN/${clang}${API}-clang"
  cargo build -p es-ffi --profile "$PROFILE" --target "$target"
  mkdir -p "$OUT/jniLibs/$abi"
  cp "target/$target/$PROFILE_DIR/libes_ffi.so" "$OUT/jniLibs/$abi/"
  # ONNX Runtime 与 llama.cpp 依赖 NDK 的共享 C++ 运行时，需随 APK 分发
  cp "$BIN/../sysroot/usr/lib/$target/libc++_shared.so" "$OUT/jniLibs/$abi/"
done

# 绑定与目标平台无关；release 产物已 strip 掉 UniFFI 元数据，从未 strip 的主机构建生成
cargo build -q -p es-ffi
cargo run -q -p es-ffi --features bindgen --bin uniffi-bindgen -- generate \
  --library "target/debug/libes_ffi.$([ "$(uname -s)" = Darwin ] && echo dylib || echo so)" \
  --language kotlin --no-format --out-dir "$OUT/kotlin"
# UniFFI 的已知问题：错误枚举字段名为 message 时与 Throwable.message 冲突，Kotlin 编译失败。
# 改成直接 override 构造参数（不改 Rust 接口）。
perl -0pi -e 's/\n(\s*)val `message`: kotlin\.String\n(\s*)\) : SpeakerException\(\) \{\n\s*override val message\n\s*get\(\) = "message=\$\{ `message` \}"\n/\n$1override val `message`: kotlin.String\n$2) : SpeakerException() {\n/g' \
  "$OUT/kotlin/app/englishspeaker/core/es_ffi.kt"
echo "✅ $OUT"
