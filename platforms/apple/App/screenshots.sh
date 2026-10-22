#!/usr/bin/env bash
# v2 界面截图：构建、安装到模拟器，用启动参数逐页截图到 docs/screenshots/v2/ios/（夜间 + 白天）。
# 用法：platforms/apple/App/screenshots.sh [模拟器 UDID]
# 端到端（真实点按）另见 EnglishSpeakerUITests：xcodebuild … test
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
U="${1:-B1D8CCF2-1C7F-41C5-A00B-BDC0A328C2A6}"
OUT="$ROOT/docs/screenshots/v2/ios"
DD="${DERIVED_DATA:-$ROOT/target/apple-app}"
BID=app.englishspeaker.ios
mkdir -p "$OUT"

(cd "$HERE" && xcodegen generate -q)
xcodebuild -project "$HERE/EnglishSpeaker.xcodeproj" -scheme EnglishSpeaker \
  -destination "id=$U" -derivedDataPath "$DD" build -quiet
xcrun simctl boot "$U" 2>/dev/null || true
xcrun simctl bootstatus "$U" -b >/dev/null
xcrun simctl status_bar "$U" override --time 9:41 --batteryState charged --batteryLevel 100 --cellularBars 4 --wifiBars 3
xcrun simctl install "$U" "$DD/Build/Products/Debug-iphonesimulator/EnglishSpeaker.app"

# shot <名称> <等待秒数> <启动参数…>
shot() {
  local name=$1 wait=$2; shift 2
  xcrun simctl terminate "$U" "$BID" 2>/dev/null || true
  xcrun simctl launch "$U" "$BID" -AppleLanguages "(zh-Hans)" -AppleLocale zh_CN "$@" >/dev/null
  sleep "$wait"
  xcrun simctl io "$U" screenshot "$OUT/$name.png" >/dev/null 2>&1
  echo "📸 $name"
}

# 资料库的多种合成状态：清空后预置 6 篇（导入即排队合成），等前两篇转完后
# 钉住一篇（已保存）、删除一篇的缓存（未转换），其余仍在 转换中 / 排队中
xcrun simctl terminate "$U" "$BID" 2>/dev/null || true
xcrun simctl launch "$U" "$BID" -resetData YES >/dev/null
sleep 7
shot 02-library-states-dark 2 -theme dark -uiTestScreen library -pin 4 -removeDownload 5
shot 02-library-states-light 1 -theme light -uiTestScreen library
sleep 60 # 等全部合成完

for theme in dark light; do
  T=(-theme "$theme")
  shot "01-home-$theme" 4 "${T[@]}" -uiTestScreen home
  shot "03-bar-A-$theme" 6 "${T[@]}" -uiTestScreen library -autoplayTitle "Why We Should Walk" -pauseAfter 2
  shot "04-bar-B-$theme" 6 "${T[@]}" -uiTestScreen library -bar B
  shot "05-bar-C-$theme" 7 "${T[@]}" -uiTestScreen library -bar C
  shot "06-player-$theme" 5 "${T[@]}" -uiTestScreen player -autoplayTitle "Why We Should Walk" -seekSentence 2 -pref.focusMode NO
  shot "07-focus-$theme" 5 "${T[@]}" -uiTestScreen player -autoplayTitle "Why We Should Walk" -seekSentence 3 -focusNow YES
  shot "08-word-sheet-$theme" 6 "${T[@]}" -uiTestScreen player -autoplayTitle "Pick It Up" -seekSentence 0 -pauseAfter 1 -wordSheet 0
  shot "09-import-$theme" 4 "${T[@]}" -uiTestScreen import -importTitle "The Quiet Power of Mornings" \
    -importBody "Most successful people have one thing in common: they protect the first hour of the day. Before the emails and the meetings, they read, walk, or simply sit with a cup of coffee.

Mornings are quiet. That quiet is not empty — it is room to think."
  shot "10-settings-$theme" 4 "${T[@]}" -uiTestScreen settings
  shot "11-settings-cache-$theme" 5 "${T[@]}" -uiTestScreen settings -scrollTo cache
  shot "12-reading-settings-$theme" 5 "${T[@]}" -uiTestScreen player -autoplayTitle "Why We Should Walk" -pauseAfter 1 -playerSheet reading
  shot "13-up-next-$theme" 5 "${T[@]}" -uiTestScreen player -autoplay 0 -pauseAfter 1 -playerSheet queue
  shot "14-search-$theme" 4 "${T[@]}" -uiTestScreen search -searchQuery walk
done
xcrun simctl terminate "$U" "$BID" 2>/dev/null || true
echo "✅ $OUT"
