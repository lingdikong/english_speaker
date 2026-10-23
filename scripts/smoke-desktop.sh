#!/usr/bin/env bash
# 桌面发布包冒烟测试：启动打包好的应用，通过 DevTools 端口确认 CEF 窗口已加载打包内的 Web UI，
# 再点页面上的关闭按钮，确认应用正常退出（需要 node）。
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
    # 点页面上的关闭按钮（macOS 红绿灯 / Windows、Linux 右上角），应用应正常退出
    ws="$(node -e 'const p=JSON.parse(process.argv[1]).find(p=>p.url.includes("es.invalid"));console.log(p.webSocketDebuggerUrl)' "$pages")"
    sleep 2
    node -e '
      const ws = new WebSocket(process.argv[1]);
      const expr = `(()=>{const b=document.querySelector(".cap.close, .light.close");if(!b)return "no close button";b.click();return "clicked " + b.className})()`;
      ws.onopen = () => ws.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression: expr, returnByValue: true } }));
      ws.onmessage = (m) => { const v = JSON.parse(m.data).result?.result?.value; if (v) { console.log(v); process.exit(v.startsWith("clicked") ? 0 : 1); } };
      ws.onerror = () => process.exit(0); // 应用退出时连接可能先断开
    ' "$ws"
    for _ in $(seq 1 20); do
      kill -0 "$PID" 2>/dev/null || { echo "✅ closed by window button"; trap - EXIT; exit 0; }
      sleep 1
    done
    echo "❌ app still running after clicking close"; cat "$LOG"; exit 1
  fi
  sleep 2
done
echo "❌ UI did not load within ${TIMEOUT}s"; echo "$pages"; cat "$LOG"; exit 1
