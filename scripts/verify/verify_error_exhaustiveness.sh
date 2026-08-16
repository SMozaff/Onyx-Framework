#!/bin/bash
# ======================================================================
# Verify error/command/event enums are matched exhaustively.
#
# Source: Verification Scripts §4; Part II Chapter 1 §1.15.
#
# Every domain error enum (MissionError, TaskError, DomainError, ...) and
# every command/event enum (MissionCommand, TaskEvent, ...) is a closed,
# frozen-spec-defined set of variants that downstream code is expected to
# handle one by one. A `match` over one of these types that ends in a
# wildcard arm (`_ => ...`, or an equivalent catch-all identifier binding
# such as `other => ...`) compiles cleanly today and keeps compiling
# tomorrow when a new variant is added — the compiler's exhaustiveness
# check, which is exactly the mechanism meant to catch "you forgot to
# handle this", is defeated by the wildcard. The new variant is silently
# routed into whatever the old default case did (often a generic 500 /
# "unknown error" / no-op), instead of failing to build until someone
# decides what the new case actually means. That is the failure this
# script exists to catch: not a crash, not a test failure, just quietly
# wrong behavior for every case added after the wildcard was written.
#
# NOTE on the original script in the Verification Scripts document: it
# ran `cargo clippy -- -D clippy::wildcard_enum_match` and inverted the
# grep, reporting success when clippy DID error. That is backwards, and
# even fixed it would not have worked: `wildcard_enum_match_arm` is a
# restriction lint that is allow-by-default and clippy does not enable it
# implicitly. This script instead does a static scan of match arms over
# the enums the frozen spec actually defines as domain error / command /
# event types, and deliberately excludes matches over foreign/std enums
# (Option, Result, StatusCode, ...) where a wildcard is legitimate --
# those types are not frozen contracts this repo owns, and a new variant
# in them is not this repo's exhaustiveness problem.
# ======================================================================

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

FAILED=0

echo "Verifying exhaustive matching over domain error/command/event enums..."
echo ""

# ----------------------------------------------------------------------
# 1. Build the set of enum names this check owns.
#
# These are `pub enum` types defined inside crates/ whose name ends in
# Error, Command, or Event -- i.e. the frozen spec's domain error types
# (MissionError, TaskError, DomainError, RepositoryError, CommandError,
# ...) and its command/event enums (MissionCommand, TaskEvent, ...).
# Built from the tree itself rather than hand-maintained, so a new
# aggregate's error/command/event enum is picked up automatically.
# Definitions under a /tests/ directory are excluded -- those are test
# fixtures, not the contract types themselves.
# ----------------------------------------------------------------------
ENUM_NAMES=$(grep -rhoE '^pub enum [A-Za-z0-9_]+(Error|Command|Event)\b' \
        --include=*.rs crates/ 2>/dev/null \
    | awk '{print $3}' \
    | sort -u)

if [ -z "$ENUM_NAMES" ]; then
    echo "  No error/command/event enums found under crates/."
    echo "  This check cannot pass vacuously -- treating as failure."
    exit 1
fi

echo "  [1/2] enums in scope:"
echo "$ENUM_NAMES" | sed 's/^/    - /'
echo ""

# Alternation regex for "does this block reference one of our enums",
# e.g. "(MissionError|TaskError|DomainError|...)::".
ENUM_ALT=$(echo "$ENUM_NAMES" | paste -sd '|' -)

# ----------------------------------------------------------------------
# 2. Scan every match block in crates/ for a wildcard-style catch-all arm
#    over one of those enums.
#
# A match block is identified as "over" one of our enums if any arm in
# it is written as `EnumName::Variant(...)` for one of the names above --
# the qualified-path style every sample in this codebase actually uses
# (no `use Enum::*` glob imports were found). Comments are stripped
# first so a doc comment mentioning a wildcard does not trip the check.
#
# A wildcard-style arm is either the literal `_ => ...` (or `_ if ... =>
# ...`), or a bare lowercase-identifier arm with no `::` and no `(` --
# in Rust match syntax a lone identifier pattern is always an irrefutable
# binding, so `other => ...` as the last arm catches every variant not
# already matched just as `_ => ...` would, whatever the binding is
# named.
# ----------------------------------------------------------------------
echo "  [2/2] scanning match blocks for wildcard arms over in-scope enums"
echo ""

TMP_HITS="$(mktemp)"
trap 'rm -f "$TMP_HITS"' EXIT

FILES=$(find crates -name '*.rs' -not -path '*/target/*' -not -path '*/tests/*' | sort)

for file in $FILES; do
    # Strip // line comments so a comment referencing an enum or the
    # word "wildcard" cannot trip the block detector or the arm checks.
    sed -e 's://.*::' "$file" | awk -v fname="$file" -v enum_alt="$ENUM_ALT" '
        BEGIN {
            enum_re = "(" enum_alt ")::"
        }
        /match[ \t]+.*\{[ \t]*$/ {
            start = FNR
            depth = 1
            block = $0 "\n"
            while (depth > 0) {
                if ((getline line) <= 0) break
                block = block line "\n"
                o = gsub(/\{/, "{", line)
                c = gsub(/\}/, "}", line)
                depth += o - c
            }
            if (block ~ enum_re) {
                n = split(block, lines, "\n")
                for (i = 1; i <= n; i++) {
                    l = lines[i]
                    # Trim leading whitespace.
                    sub(/^[ \t]+/, "", l)
                    is_underscore = (l ~ /^_[ \t]*(if[^=]*)?=>/)
                    is_bare_ident = (l ~ /^[a-z_][A-Za-z0-9_]*[ \t]*(if[^=]*)?=>/)
                    if (is_underscore || is_bare_ident) {
                        print fname ":" (start + i - 1) ": " l
                    }
                }
            }
        }
    '
done > "$TMP_HITS"

if [ -s "$TMP_HITS" ]; then
    echo "    [FAIL] wildcard-style arm(s) in a match over an in-scope enum:"
    sed 's/^/      /' "$TMP_HITS"
    FAILED=1
else
    echo "    ok"
fi

echo ""
if [ "$FAILED" -eq 0 ]; then
    echo "[PASS] All matches over domain error/command/event enums are exhaustive"
    exit 0
else
    echo "[FAIL] Non-exhaustive match(es) found over a domain error/command/event enum"
    echo "       Replace the wildcard/catch-all arm with one arm per variant so a"
    echo "       future variant fails to compile here until it is handled (Part II"
    echo "       Ch.1 §1.15)."
    exit 1
fi
