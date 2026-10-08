#!/bin/bash
# ======================================================================
# Verify logging is structured JSON carrying the mandatory R3 fields.
#
# Source: Verification Scripts §12; Part II Chapter 1 §1.18.1; Team
# Prompt 7 §3.8. §3.8 lists the mandatory fields every log line must
# carry: timestamp, level, service, trace_id, operation_id,
# correlation_id, actor_id, organization_id, command_type, outcome,
# latency_ms, error_code, message.
#
# Why this matters: incident response and audit reconstruction both
# depend on being able to grep/query logs by these fields across every
# service. A log line emitted as free text, or missing a field the
# on-call runbook assumes is always there, does not fail loudly — it
# just fails to show up in the query when someone needs it during an
# incident, which is the worst possible time to discover the schema was
# never enforced.
# ======================================================================

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

FAILED=0
ADAPTER_SRC="crates/infrastructure/observability-adapter/src"
LOGGING_FILE="$ADAPTER_SRC/logging.rs"
TRACING_FILE="$ADAPTER_SRC/tracing.rs"

# Team Prompt 7 §3.8's mandatory field list, verbatim.
MANDATORY_FIELDS=(
    timestamp level service trace_id operation_id correlation_id
    actor_id organization_id command_type outcome latency_ms
    error_code message
)

echo "Verifying structured JSON logging (Team Prompt 7 §3.8)..."
echo ""

# ----------------------------------------------------------------------
# 1. A JSON formatter, not a plain/pretty text formatter, must be wired
#    into the global subscriber that observability-adapter installs.
#
# Part II Chapter 1 §1.18.1 requires every log line to be one parseable
# JSON object. tracing_subscriber's default/`pretty`/`compact` fmt
# layers emit human-readable text, which breaks any log pipeline that
# parses lines as JSON. Comments are stripped first so a doc comment
# describing the rule does not itself trip the check.
# ----------------------------------------------------------------------
echo "  [1/3] observability-adapter wires a JSON formatter, not plain text"
if [ ! -f "$TRACING_FILE" ]; then
    echo "    [FAIL] $TRACING_FILE not found"
    FAILED=1
else
    stripped=$(sed -e 's://.*::' "$TRACING_FILE")
    if echo "$stripped" | grep -qE 'tracing_subscriber::fmt(\(\)|::| )|fmt::layer\(\)|\.pretty\(\)|\.compact\(\)'; then
        echo "    [FAIL] $TRACING_FILE wires a plain/pretty text fmt layer"
        FAILED=1
    elif ! echo "$stripped" | grep -qE 'CanonicalJsonLayer'; then
        echo "    [FAIL] $TRACING_FILE does not install CanonicalJsonLayer"
        FAILED=1
    else
        echo "    ok"
    fi
fi

# ----------------------------------------------------------------------
# 2. Every mandatory field from §3.8 must be a key the logging layer
#    actually emits.
#
# Scoped to the json!({...}) record literal built inside
# CanonicalJsonLayer::on_event — the one expression whose keys are what
# actually gets serialized onto the wire. Grepping the whole file (or
# whole crate) for these names is not enough: logging.rs may also define
# unrelated structs, dead code, or string constants that happen to
# contain the same field-name text (e.g. an unrelated diagnostic struct
# with a `correlation_id` field, or a stray string literal "error_code")
# without that field ever reaching the emitted record. Only text inside
# the json!() macro call counts as evidence the field is emitted.
# ----------------------------------------------------------------------
echo "  [2/3] logging layer emits every §3.8 mandatory field"
if [ ! -f "$LOGGING_FILE" ]; then
    echo "    [FAIL] $LOGGING_FILE not found"
    FAILED=1
else
    stripped=$(sed -e 's://.*::' "$LOGGING_FILE")
    record_block=$(echo "$stripped" | awk '/let record = json!\(/{flag=1} flag{print} flag && /^\s*\}\);/{exit}')
    if [ -z "$record_block" ]; then
        echo "    [FAIL] $LOGGING_FILE has no 'let record = json!(' block to check"
        FAILED=1
        missing=("${MANDATORY_FIELDS[@]}")
    else
        missing=()
        for field in "${MANDATORY_FIELDS[@]}"; do
            if ! echo "$record_block" | grep -qE "\"$field\""; then
                missing+=("$field")
            fi
        done
    fi
    if [ "${#missing[@]}" -gt 0 ]; then
        echo "    [FAIL] $LOGGING_FILE is missing mandatory field(s): ${missing[*]}"
        FAILED=1
    else
        echo "    ok"
    fi
fi

# ----------------------------------------------------------------------
# 3. No println!/eprintln! bypassing the structured layer inside crates/.
#
# A println! anywhere in library/domain/application/infrastructure code
# writes an unstructured line straight to stdout, invisible to the JSON
# layer and to anything that queries logs by the §3.8 fields. Excluded:
#   - crates/bins/** — a bin's own startup/CLI output (before the
#     subscriber is installed, or interactive CLI UX) is not a service
#     log line and is legitimate here.
#   - build.rs — build-script output is a cargo build-time protocol
#     (`cargo:warning=...`), not application logging.
#   - test code — #[cfg(test)] modules and files under a tests/
#     directory, where println! is normal test-debugging output that
#     never runs in production.
# ----------------------------------------------------------------------
echo "  [3/3] no println!/eprintln! bypassing structured logging in crates/"
hits=""
scanned=0
while IFS= read -r -d '' file; do
    scanned=$((scanned + 1))
    case "$file" in
        */bins/*) continue ;;
        */tests/*) continue ;;
        *build.rs) continue ;;
        *_test.rs|*test_*.rs) continue ;;
    esac
    # Drop any #[cfg(test)] module tail before matching, so in-file test
    # modules under a non-test/ file (e.g. `mod tests { ... }` at the
    # bottom of a source file) are not flagged for their println! calls.
    body=$(awk '/#\[cfg\(test\)\]/{exit} {print}' "$file")
    match=$(echo "$body" | sed -e 's://.*::' | grep -nE '(^|[^a-zA-Z_])(println!|eprintln!)')
    if [ -n "$match" ]; then
        hits="$hits\n$file:\n$(echo "$match" | sed 's/^/      /')"
    fi
done < <(find crates -name '*.rs' -print0)

# A glob that silently matched nothing is not a passing check — it is an
# unchecked check. If crates/ is missing or contains no .rs files (bad
# cwd, broken checkout, workspace layout change), fail loudly instead of
# reporting "ok" for zero files scanned.
if [ "$scanned" -eq 0 ]; then
    echo "    [FAIL] found no .rs files under crates/ — nothing was scanned"
    FAILED=1
elif [ -n "$hits" ]; then
    echo "    [FAIL] println!/eprintln! bypassing structured logging:"
    echo -e "$hits" | sed 's/^/    /'
    FAILED=1
else
    echo "    ok"
fi

echo ""
if [ $FAILED -eq 0 ]; then
    echo "[PASS] Structured JSON logging conforms to Team Prompt 7 §3.8"
    exit 0
else
    echo "[FAIL] R3 structured logging contract violation"
    echo "       Every log line must be one JSON object carrying all of:"
    echo "       ${MANDATORY_FIELDS[*]}"
    echo "       (Team Prompt 7 §3.8, Part II Chapter 1 §1.18.1)."
    exit 1
fi
