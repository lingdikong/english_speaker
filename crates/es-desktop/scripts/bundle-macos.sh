#!/usr/bin/env bash
# 打包 macOS 桌面应用：English Speaker.app（Rust 宿主 + CEF + Web UI）。
#
#   crates/es-desktop/scripts/bundle-macos.sh [--release] [--copy-models] [-o <输出目录>]
#
# 布局（CEF 要求）：
#   es-desktop.app/Contents/MacOS/es-desktop
#   es-desktop.app/Contents/Frameworks/Chromium Embedded Framework.framework
#   es-desktop.app/Contents/Frameworks/es-desktop Helper{, (GPU), (Renderer), (Plugin), (Alerts)}.app
#   es-desktop.app/Contents/Resources/ui       ← ui/web/dist
#   es-desktop.app/Contents/Resources/models   ← 仓库 models（默认软链接，--copy-models 复制）
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
PROFILE=debug
CARGO_FLAGS=()
COPY_MODELS=0
OUT="$ROOT/target/bundle"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE=release; CARGO_FLAGS+=(--release) ;;
    --copy-models) COPY_MODELS=1 ;;
    -o) OUT="$2"; shift ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done

NAME=es-desktop
DISPLAY_NAME="English Speaker"
BUNDLE_ID=app.englishspeaker.desktop
VERSION="${ES_VERSION:-0.1.0}"

# 1. Web UI
( cd "$ROOT/ui/web" && { [[ -d node_modules ]] || npm ci; } && npm run build )

# 2. Rust（主程序 + helper）
cargo build -p es-desktop --features cef --bins ${CARGO_FLAGS[@]+"${CARGO_FLAGS[@]}"} --manifest-path "$ROOT/Cargo.toml"
BIN="$ROOT/target/$PROFILE"

# 3. cef-dll-sys 构建时下载的 CEF 二进制（或 CEF_PATH 指定的目录）
case "$(uname -m)" in arm64) ARCH=aarch64 ;; *) ARCH=x86_64 ;; esac
CEF_DIR="${CEF_PATH:-}"
if [[ -z "$CEF_DIR" ]]; then
  CEF_DIR="$(ls -dt "$ROOT"/target/$PROFILE/build/cef-dll-sys-*/out/cef_macos_$ARCH 2>/dev/null | head -1)"
fi
FRAMEWORK="$CEF_DIR/Chromium Embedded Framework.framework"
[[ -d "$FRAMEWORK" ]] || { echo "CEF framework not found under $CEF_DIR" >&2; exit 1; }

APP="$OUT/$NAME.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Frameworks" "$APP/Contents/Resources"

plist() { # plist <executable> <is_helper>
  local exe="$1" helper="$2"
  cat <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$DISPLAY_NAME</string>
  <key>CFBundleDisplayName</key><string>$DISPLAY_NAME</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID$([[ $helper == 1 ]] && echo .helper)</string>
  <key>CFBundleExecutable</key><string>$exe</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>13.4</string>
  <key>LSEnvironment</key><dict><key>MallocNanoZone</key><string>0</string></dict>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>NSRequiresAquaSystemAppearance</key><false/>
$([[ $helper == 1 ]] && echo '  <key>LSUIElement</key><string>1</string>')
</dict>
</plist>
PLIST
}

cp "$BIN/$NAME" "$APP/Contents/MacOS/$NAME"
plist "$NAME" 0 > "$APP/Contents/Info.plist"
cp -R "$FRAMEWORK" "$APP/Contents/Frameworks/"

for suffix in "" " (GPU)" " (Renderer)" " (Plugin)" " (Alerts)"; do
  H="$NAME Helper$suffix"
  mkdir -p "$APP/Contents/Frameworks/$H.app/Contents/MacOS"
  cp "$BIN/$NAME-helper" "$APP/Contents/Frameworks/$H.app/Contents/MacOS/$H"
  plist "$H" 1 > "$APP/Contents/Frameworks/$H.app/Contents/Info.plist"
done

cp -R "$ROOT/ui/web/dist" "$APP/Contents/Resources/ui"
if [[ -d "$ROOT/models" ]]; then
  if [[ $COPY_MODELS == 1 ]]; then
    cp -R "$ROOT/models" "$APP/Contents/Resources/models"
  else
    ln -s "$ROOT/models" "$APP/Contents/Resources/models"
  fi
fi

# 本地运行用的临时签名（未签名的 framework 在新系统上可能被拒绝加载）
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
echo "bundled: $APP"
