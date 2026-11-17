#!/bin/zsh
# Builds the app, replaces ~/Applications/Transcribe Lecture.app and starts it, without ever
# interrupting a lecture:
#
#   scripts/install.sh            if the app is open but idle, quit it first (it saves), then reopen
#   scripts/install.sh --no-open  don't start it afterwards
#
# It refuses, changing nothing, while the app is recording or paused. The app writes its state
# ("recording", "paused", "idle") to <data dir>/state; builds older than that are judged by
# whether a transcript was written in the last minute.
set -eu
cd "$(dirname "$0")/.."
export SDKROOT="${SDKROOT:-$(ls -d /Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk 2>/dev/null || xcrun --show-sdk-path)}"
NAME="Transcribe Lecture"
DATA=~/Library/Application\ Support/$NAME
[[ -d $DATA ]] || DATA=~/Library/Application\ Support/Lecture\ Scribe
LECTURES=~/Documents/Lecture\ Transcripts

running() { pgrep -f "$NAME.app/Contents/MacOS/|Lecture Scribe.app/Contents/MacOS/" >/dev/null; }
busy() {
  running || return 1
  local st=$(cat "$DATA/state" 2>/dev/null || true)
  [[ $st == recording || $st == paused ]] && return 0
  # No state file: an older build. A transcript or WAV written in the last minute means it's recording.
  [[ -z $st && -n $(find "$LECTURES" \( -name '*.md' -o -name '*.wav' \) -mtime -1m 2>/dev/null | head -1) ]] && return 0
  return 1
}

if busy; then
  echo "A lecture is being recorded (or is paused). Nothing was changed: stop it, then run this again." >&2
  exit 1
fi
nice -n 10 ./bundle.sh
if running; then
  busy && { echo "A recording started during the build. Nothing was installed." >&2; exit 1; }
  echo "Quitting $NAME (idle)…"
  osascript -e "tell application \"$NAME\" to quit" >/dev/null 2>&1 || true
  for _ in {1..30}; do running || break; sleep 0.5; done
  running && { echo "$NAME didn't quit. Nothing was installed." >&2; exit 1; }
fi
mkdir -p ~/Applications
rm -rf ~/Applications/"$NAME.app"
cp -R "target/$NAME.app" ~/Applications/
echo "Installed ~/Applications/$NAME.app"
[[ ${1:-} == --no-open ]] || { open ~/Applications/"$NAME.app"; echo "Started $NAME."; }
