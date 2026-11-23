#!/bin/zsh
# Runs every check. Use it before saying a change is done.
#
#   scripts/verify.sh            everything (about a minute)
#   scripts/verify.sh --quick    skip the engine tests (no speech model needed)
#   scripts/verify.sh --ui NAME  only the UI tests whose file or name contains NAME
#
# Steps: release build with no warnings, unit tests, UI script syntax, UI tests in WebKit (the
# app's real page and an exported deck, driven by real mouse and key events: tests/ui), engine tests (Parakeet on
# speech made with `say`: tests/engine.rs).
#
# Safe while a lecture is recording: nothing touches the installed app, the user's lectures or
# settings, and the builds run at low priority. UI screenshots go to target/ui-test/shots/.
set -u
setopt pipefail
cd "$(dirname "$0")/.."
export SDKROOT="${SDKROOT:-$(ls -d /Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk 2>/dev/null || xcrun --show-sdk-path)}"
QUICK=0; ONLY=()
while (( $# )); do
  case "$1" in
    --quick) QUICK=1 ;;
    --ui) ONLY=(--only "$2"); shift ;;
    *) echo "unknown option $1"; exit 2 ;;
  esac
  shift
done
# Low priority, so a recording in the installed app isn't starved.
CARGO=(nice -n 20 cargo)
pgrep -fq "Transcribe Lecture.app|Lecture Scribe.app" && export CARGO_BUILD_JOBS=2

typeset -a RESULTS; FAILED=0
step() { print -P "\n%B== $1%b"; }
pass() { RESULTS+=("✓ $1"); }
fail() { RESULTS+=("✗ $1"); FAILED=1; }
OUT=target/ui-test; mkdir -p $OUT

step "build (release, no warnings)"
"${CARGO[@]}" build --release --all-targets > $OUT/build.log 2>&1
if (( $? )); then fail "build"; grep -E -A12 "^error" $OUT/build.log | head -60
elif grep -q "^warning" $OUT/build.log; then fail "build has warnings"; grep -E -A8 "^warning" $OUT/build.log | head -60
else pass "build"; fi

step "unit tests"
if "${CARGO[@]}" test --release --lib -q 2>&1 | tail -3; then pass "unit tests"; else fail "unit tests"; fi

step "UI script syntax"
python3 - <<'EOF' > $OUT/ui.js
import re
s = open("src/ui.html").read()
print(max(re.findall(r"<script>(.*?)</script>", s, re.S), key=len).replace("/*SLIDES_JS*/", ""))
EOF
if node --check $OUT/ui.js; then pass "UI script syntax"; else fail "UI script syntax"; fi

step "UI tests (WebKit)"
target/release/transcribe-lecture --dump-ui > $OUT/page.html
RUNNER=$OUT/runner
if [[ ! -x $RUNNER || tests/ui/runner.swift -nt $RUNNER ]]; then
  swiftc -O -suppress-warnings tests/ui/runner.swift -o $RUNNER || fail "UI runner doesn't compile"
fi
rm -rf $OUT/shots
if [[ -x $RUNNER ]] && $RUNNER $OUT/page.html tests/ui/prelude.js $OUT/shots tests/ui/*.test.js $ONLY; then pass "UI tests"; else fail "UI tests"; fi
# The exported deck (presenting, builds), from a fixture deck.
target/release/transcribe-lecture --dump-deck tests/ui/deck/slides.json > $OUT/deck.html
if [[ -x $RUNNER ]] && $RUNNER $OUT/deck.html tests/ui/prelude.js $OUT/shots tests/ui/deck/*.test.js $ONLY; then pass "deck UI tests"; else fail "deck UI tests"; fi

if (( QUICK )); then
  RESULTS+=("- engine tests skipped (--quick)")
else
  step "engine tests (Parakeet + say)"
  if "${CARGO[@]}" test --release --test engine -q -- --ignored --test-threads 1 2>&1 | tail -15; then pass "engine tests"; else fail "engine tests"; fi
fi

print -P "\n%B== summary%b"
printf '%s\n' "${RESULTS[@]}"
[[ -d $OUT/shots ]] && echo "UI screenshots: $OUT/shots/"
exit $FAILED
