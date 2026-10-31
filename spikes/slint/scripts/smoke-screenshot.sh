#!/usr/bin/env bash
# Smoke test: the window starts under a virtual X display, stays up, and can be captured.
# Usage: BIN=/path/to/onyx-slint-spike OUT=/path/to/shot.png xvfb-run -a scripts/smoke-screenshot.sh
set -u
BIN="${BIN:?set BIN}"
OUT="${OUT:-shot.png}"
"$BIN" > "${OUT%.png}.log" 2>&1 &
pid=$!
sleep "${WAIT:-6}"
if kill -0 "$pid" 2>/dev/null; then
  alive=yes
else
  alive=no
fi
if command -v import >/dev/null 2>&1; then
  import -window root "$OUT" && captured=yes || captured=no
else
  captured=no
fi
kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null; code=$?
echo "still_running_after_wait=$alive captured=$captured exit_after_kill=$code"
[ "$alive" = yes ] && [ "$captured" = yes ]
