#!/usr/bin/env bash
# U-10 check (Linux): reads the OS animation setting through the spike binary and
# checks the resolved value that reaches Slint's Motion global.
# Uses an isolated GSettings keyfile backend and XDG config dir, so the host
# desktop settings are not changed. Needs: gsettings and a DISPLAY.
# Usage: BIN=/path/to/onyx-slint-spike xvfb-run -a scripts/motion-check.sh
set -u
BIN="${BIN:-$(dirname "$0")/../target/debug/onyx-slint-spike}"
WORK="${WORK:-$(mktemp -d)}"
pass=0; fail=0

if [ -z "${DISPLAY:-}" ]; then
  echo "Run this script under a display, for example: xvfb-run -a $0" >&2
  exit 2
fi

run_report() { # $1 = config dir, extra env via remaining args
  local cfg="$1"; shift
  env "$@" XDG_CONFIG_HOME="$cfg" "$BIN" --motion-report
}

check() { # $1 = name, $2 = output, $3 = expected substring
  if printf '%s' "$2" | grep -q -F "$3"; then
    echo "PASS  $1  -> $2"; pass=$((pass+1))
  else
    echo "FAIL  $1  -> got: $2   expected: $3"; fail=$((fail+1))
  fi
}

# Case 1 and 2: GNOME setting set to false, then true (toggle). Keyfile backend.
CFG="$WORK/gnome"; mkdir -p "$CFG"
export GSETTINGS_BACKEND=keyfile
gsettings_set() { XDG_CONFIG_HOME="$CFG" gsettings set org.gnome.desktop.interface enable-animations "$1"; }
gsettings_set false
out=$(run_report "$CFG" GSETTINGS_BACKEND=keyfile)
check "gnome=false -> reduced in Slint" "$out" '"resolved": "reduced", "slint_reduced": true'
gsettings_set true
out=$(run_report "$CFG" GSETTINGS_BACKEND=keyfile)
check "gnome=true (toggle) -> full in Slint" "$out" '"resolved": "full", "slint_reduced": false'
unset GSETTINGS_BACKEND

# Case 3: setting unreadable (no gsettings on PATH, no KDE file) -> reduced (U-10 fallback).
CFG="$WORK/none"; mkdir -p "$CFG"
out=$(run_report "$CFG" PATH=/nonexistent)
check "unreadable setting -> reduced (fallback)" "$out" '"os_animations": null, "resolved": "reduced", "slint_reduced": true'

# Case 4: KDE setting via kdeglobals (no gsettings on PATH).
CFG="$WORK/kde-off"; mkdir -p "$CFG"
printf '[KDE]\nAnimationDurationFactor=0\n' > "$CFG/kdeglobals"
out=$(run_report "$CFG" PATH=/nonexistent)
check "KDE factor=0 -> reduced" "$out" '"resolved": "reduced"'

CFG="$WORK/kde-on"; mkdir -p "$CFG"
printf '[KDE]\nAnimationDurationFactor=1\n' > "$CFG/kdeglobals"
out=$(run_report "$CFG" PATH=/nonexistent)
check "KDE factor=1 -> full" "$out" '"resolved": "full", "slint_reduced": false'

# Case 5: corrupt KDE value is unreadable -> reduced.
CFG="$WORK/kde-bad"; mkdir -p "$CFG"
printf '[KDE]\nAnimationDurationFactor=lots\n' > "$CFG/kdeglobals"
out=$(run_report "$CFG" PATH=/nonexistent)
check "corrupt KDE value -> reduced (fallback)" "$out" '"resolved": "reduced"'

echo "----"
echo "passed=$pass failed=$fail"
[ "$fail" -eq 0 ]
