#!/usr/bin/env bash
# Makes the README screenshot, docs/screenshot.png, from a real lecture: the app's page in a
# real window, staged as it looks while recording that lecture (scripts/screenshot.js).
#
#   scripts/screenshot.sh <lecture.md> <HH:MM:SS> <slide number> [out.png]
#
# HH:MM:SS is how far into the recording; the slide is the one shown on the right. It doesn't
# touch the installed app, the lecture, or your settings. A window comes to the front for a couple
# of seconds, then focus goes back. The terminal needs Screen Recording permission (for `screencapture`).
set -euo pipefail
cd "$(dirname "$0")/.."
[[ $# -ge 3 ]] || { sed -n 4,6p "$0" >&2; exit 2; }
LECTURE=$1 AT=$2 SLIDE=$3 OUT=${4:-docs/screenshot.png}
DIR=target/screenshot; mkdir -p $DIR

cargo build --release -q
target/release/transcribe-lecture --dump-ui > $DIR/page.html
target/release/transcribe-lecture --dump-lecture "$LECTURE" > $DIR/lecture.json
IFS=: read -r H M S <<< "$AT"
CUT=$(( 10#$H * 3600 + 10#$M * 60 + 10#$S ))
printf '{"data":%s,"cut":%d,"slide":%d,"device":"MacBook Pro Microphone"}' "$(cat $DIR/lecture.json)" $CUT $SLIDE > $DIR/shot.json

BIN=$DIR/screenshot
if [[ ! -x $BIN || scripts/screenshot.swift -nt $BIN ]]; then
  swiftc -O -suppress-warnings scripts/screenshot.swift -o $BIN
fi
# Wide enough for the whole recording status in the header.
$BIN $DIR/page.html scripts/screenshot.js $DIR/shot.json "$OUT" 1240 780
echo "Wrote $OUT"
