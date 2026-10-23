#!/usr/bin/env bash
# 打包桌面发布包（Rust 宿主 + CEF + Web UI + 模型），在对应系统上运行：
#
#   ES_VERSION=0.2.0 scripts/package-desktop.sh [-o <输出目录>]
#
# 产物（dist/）：
#   macOS    EnglishSpeaker-<ver>-macos-<arch>.dmg      English Speaker.app
#   Windows  EnglishSpeaker-<ver>-windows-x86_64.zip    EnglishSpeaker/es-desktop.exe + CEF + ui/ + models/
#   Linux    EnglishSpeaker-<ver>-linux-x86_64.tar.gz   EnglishSpeaker/es-desktop    + CEF + ui/ + models/
# 并输出解压后的目录路径到 dist/APP_PATH（供冒烟测试使用）。
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="${ES_VERSION:-0.1.0}"
OUT="$ROOT/dist"
while [[ $# -gt 0 ]]; do
  case "$1" in
    -o) OUT="$2"; shift ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done
mkdir -p "$OUT"
case "$(uname -m)" in arm64|aarch64) ARCH=aarch64 ;; *) ARCH=x86_64 ;; esac
case "$(uname -s)" in
  Darwin) OS=macos ;;
  Linux) OS=linux ;;
  MINGW*|MSYS*|CYGWIN*) OS=windows ;;
  *) echo "unsupported OS $(uname -s)" >&2; exit 2 ;;
esac
BASE="EnglishSpeaker-$VERSION-$OS-$ARCH"
DOCS=(README.md LICENSE NOTICE ACKNOWLEDGEMENTS.md)

if [[ $OS == macos ]]; then
  WORK="$ROOT/target/package"
  rm -rf "$WORK" && mkdir -p "$WORK"
  ES_VERSION="$VERSION" "$ROOT/crates/es-desktop/scripts/bundle-macos.sh" --release --copy-models -o "$WORK"
  APP="$WORK/English Speaker.app"
  mv "$WORK/es-desktop.app" "$APP"
  codesign --force --deep --sign - "$APP"
  STAGE="$WORK/dmg" && mkdir -p "$STAGE"
  cp -R "$APP" "$STAGE/"
  ln -s /Applications "$STAGE/Applications"
  for f in "${DOCS[@]}"; do cp "$ROOT/$f" "$STAGE/"; done
  rm -f "$OUT/$BASE.dmg"
  hdiutil create -volname "English Speaker $VERSION" -srcfolder "$STAGE" -fs HFS+ -format UDZO -ov "$OUT/$BASE.dmg" >/dev/null
  echo "$APP" > "$OUT/APP_PATH"
  echo "packaged: $OUT/$BASE.dmg"
  exit 0
fi

# Windows / Linux：CEF 运行时文件与可执行文件放在同一目录
( cd "$ROOT/ui/web" && { [[ -d node_modules ]] || npm ci; } && npm run build )
cargo build --release -p es-desktop --features cef --bin es-desktop --manifest-path "$ROOT/Cargo.toml"
EXE=es-desktop; [[ $OS == windows ]] && EXE=es-desktop.exe
CEF_DIR="${CEF_PATH:-$(ls -dt "$ROOT"/target/release/build/cef-dll-sys-*/out/cef_${OS}_$ARCH 2>/dev/null | head -1)}"
[[ -d "$CEF_DIR" ]] || { echo "CEF runtime not found (cef_${OS}_$ARCH)" >&2; exit 1; }

WORK="$ROOT/target/package"
APPDIR="$WORK/EnglishSpeaker"
rm -rf "$WORK" && mkdir -p "$APPDIR"
cp "$ROOT/target/release/$EXE" "$APPDIR/"
# CEF 运行时：顶层文件（dll/so、pak、icudtl.dat、snapshot…）与 locales/；不要头文件、cmake、导入库
find "$CEF_DIR" -maxdepth 1 -type f ! -name '*.lib' ! -name '*.h' ! -name 'CMakeLists.txt' ! -name 'archive.json' \
  -exec cp {} "$APPDIR/" \;
cp -R "$CEF_DIR/locales" "$APPDIR/locales"
cp -R "$ROOT/ui/web/dist" "$APPDIR/ui"
cp -R "$ROOT/models" "$APPDIR/models"
find "$APPDIR/models" -name '.DS_Store' -delete
for f in "${DOCS[@]}"; do cp "$ROOT/$f" "$APPDIR/"; done

if [[ $OS == windows ]]; then
  rm -f "$OUT/$BASE.zip"
  ( cd "$WORK" && 7z a -tzip -mx=7 "$(cygpath -w "$OUT/$BASE.zip")" EnglishSpeaker >/dev/null )
  echo "packaged: $OUT/$BASE.zip"
else
  cat > "$APPDIR/english-speaker.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=English Speaker
Comment=Offline English article to podcast
Exec=es-desktop
Categories=Education;Audio;
DESKTOP
  tar -C "$WORK" -czf "$OUT/$BASE.tar.gz" EnglishSpeaker
  echo "packaged: $OUT/$BASE.tar.gz"
fi
echo "$APPDIR/$EXE" > "$OUT/APP_PATH"
