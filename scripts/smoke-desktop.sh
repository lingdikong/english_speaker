#!/usr/bin/env bash
# 桌面发布包冒烟测试：启动打包好的应用，通过 DevTools 端口确认 CEF 窗口已加载打包内的 Web UI。
#
#   scripts/smoke-desktop.sh <应用路径：.app 目录或可执行文件> [超时秒数]
# Linux 无显示器时用 `xvfb-run -a scripts/smoke-desktop.sh …`。
set -euo pipefail
APP="$1"
TIMEOUT="${2:-120}"
PORT=9339
EXE="$APP"
[[ -d "$APP" && "$APP" == *.app ]] && EXE="$APP/Contents/MacOS/es-desktop"
DATA="$(mktemp -d)"
LOG="$DATA/app.log"

ES_DATA_DIR="$DATA" "$EXE" --mute --devtools=$PORT >"$LOG" 2>&1 &
PID=$!
cleanup() {
  kill "$PID" 2>/dev/null || true
  case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) taskkill //F //IM es-desktop.exe >/dev/null 2>&1 || true ;; esac
}
trap cleanup EXIT

deadline=$((SECONDS + TIMEOUT))
while (( SECONDS < deadline )); do
  if ! kill -0 "$PID" 2>/dev/null; then
    echo "❌ app exited early"; cat "$LOG"; exit 1
  fi
  pages="$(curl -s "http://127.0.0.1:$PORT/json" || true)"
  if grep -q '"url": *"https://es.invalid/index.html"' <<<"$pages" && grep -q '"title": *"English Speaker"' <<<"$pages"; then
    echo "✅ UI loaded in CEF"
    exit 0
  fi
  sleep 2
done
echo "❌ UI did not load within ${TIMEOUT}s"; echo "$pages"; cat "$LOG"; exit 1
