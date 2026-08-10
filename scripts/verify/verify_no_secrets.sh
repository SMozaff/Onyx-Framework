#!/bin/bash
# ======================================================================
# Verify no secrets can reach events, logs, or serialized output.
#
# Source: Verification Scripts §11; Part I §3.4 (events are replicated to
# every peer and retained indefinitely for audit/replay); Part II
# Chapter 1 §1.17.1 (secrets and credentials must never be placed in a
# domain event, a log line, or any payload that crosses a replication or
# transport boundary).
#
# The failure mode this catches does not crash and does not fail a unit
# test: a password hash, bearer token, or AuthorityProof signature that
# leaks into an event payload gets CRDT-replicated to every other
# replica, written into the append-only event log, and mirrored into
# whatever log aggregator ingests tracing output — all of it durable,
# all of it outside the reach of a later fix. By the time anyone notices
# the field, it has already been copied to every device that synced.
#
# Three checks:
#   (a) AuthorityProof must never appear in a domain event definition
#       (crates/domains/*/src/event.rs) — commands legitimately carry it
#       for authorization, but once a fact is recorded as an event it is
#       replicated and retained, and the proof has no business being in
#       that payload.
#   (b) no field named password/token/secret/signature/api_key derives
#       Serialize on a type that is an event, envelope, audit, or log
#       payload.
#   (c) tracing/log macro calls must not interpolate a whole envelope,
#       payload, or anything named *proof/*secret/*token/*password —
#       only named, scalar fields may be logged.
#
# Comments are stripped before matching so a doc comment mentioning one
# of these words (e.g. explaining why a field is NOT present) cannot
# trip the check.
# ======================================================================

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

FAILED=0

strip_comments() {
    # Strips '//' line comments. Good enough for this codebase's style;
    # doc comments (///, //!) are '//' comments too and are stripped the
    # same way.
    sed -e 's://.*::'
}

echo "Verifying no secrets reach events, logs, or serialized output..."
echo ""

# ----------------------------------------------------------------------
# (a) AuthorityProof must never appear in a domain event definition.
#
# Scoped to crates/domains/*/src/event.rs specifically. platform-kernel
# legitimately DEFINES AuthorityProof (it is the type), and
# CommandEnvelope (platform-contracts/src/command.rs) legitimately
# CARRIES it (a command needs proof of authorization to be accepted).
# Neither of those is a domain event. A domain event is a recorded fact
# that gets CRDT-replicated and retained; if AuthorityProof reaches one,
# a bearer credential is now durable on every replica.
# ----------------------------------------------------------------------
echo "  [1/3] AuthorityProof does not appear in domain event definitions"
EVENT_FILES=$(find crates/domains -path '*/src/event.rs' 2>/dev/null | sort)
if [ -z "$EVENT_FILES" ]; then
    echo "    [FAIL] no domain event.rs files found under crates/domains"
    FAILED=1
else
    hit_any=0
    for f in $EVENT_FILES; do
        hits=$(strip_comments < "$f" | grep -n 'AuthorityProof' || true)
        if [ -n "$hits" ]; then
            echo "    [FAIL] $f embeds AuthorityProof in a domain event:"
            echo "$hits" | sed 's/^/      /'
            FAILED=1
            hit_any=1
        fi
    done
    if [ "$hit_any" -eq 0 ]; then
        echo "    ok ($(echo "$EVENT_FILES" | wc -l | tr -d ' ') file(s) checked)"
    fi
fi

# ----------------------------------------------------------------------
# (b) No sensitive-named field derives Serialize on an event/log/audit
#     payload type.
#
# Scoped to files that are actually event, envelope, audit, or log
# payload definitions — not every struct in the workspace that derives
# Serialize. An HTTP request/response DTO in bins/api-server (e.g.
# LoginResponse's access_token, returned directly to the client that
# just authenticated) is a transport boundary the caller is entitled to
# see, not an event or a log line; flagging it would be a false
# positive of exactly the kind this task warns against. What matters is
# whether the field can be replicated into an event or written to a
# log, so the check is scoped to the files that implement those things.
# ----------------------------------------------------------------------
echo "  [2/3] no password/token/secret/signature/api_key field on an event/log/audit payload"
PAYLOAD_FILES=$(find crates -iname '*event*.rs' -o -iname '*envelope*.rs' -o -iname '*audit*.rs' -o -iname '*log*.rs' 2>/dev/null \
    | grep -v '/target/' \
    | grep -vE '/(tests)/' \
    | sort -u)

if [ -z "$PAYLOAD_FILES" ]; then
    echo "    [FAIL] no event/envelope/audit/log source files found"
    FAILED=1
else
    hit_any=0
    for f in $PAYLOAD_FILES; do
        # Only care about files that derive Serialize somewhere at all;
        # skip the rest cheaply.
        if ! grep -q 'derive(' "$f"; then
            continue
        fi
        stripped="$(strip_comments < "$f")"
        if ! echo "$stripped" | grep -q 'Serialize'; then
            continue
        fi
        # Field declarations of the sensitive names, e.g. `password:` or
        # `pub api_key:`. Matched on the field-declaration shape (name
        # followed by a colon) so prose mentioning the word elsewhere in
        # the file does not trip it.
        hits=$(echo "$stripped" | grep -nE '^\s*(pub(\([^)]*\))?\s+)?(password|token|secret|signature|api_key)\s*:' || true)
        if [ -n "$hits" ]; then
            echo "    [FAIL] $f has a sensitive field on a Serialize-deriving type:"
            echo "$hits" | sed 's/^/      /'
            FAILED=1
            hit_any=1
        fi
    done
    if [ "$hit_any" -eq 0 ]; then
        echo "    ok ($(echo "$PAYLOAD_FILES" | wc -l | tr -d ' ') file(s) checked)"
    fi
fi

# ----------------------------------------------------------------------
# (c) tracing/log macro calls must not interpolate a whole envelope,
#     payload, or anything named *proof/*secret/*token/*password.
#
# A call like `tracing::error!(?envelope, ...)` or
# `tracing::info!(%command.authority_proof, ...)` puts the entire
# structure — proof, signature, and all — into the log line verbatim.
# Logging a single scalar field pulled off such a value (e.g.
# `operation_id = %envelope.operation_id`) is fine and common in this
# codebase; the violation is interpolating the container itself, or any
# value whose own name says it is a secret.
# ----------------------------------------------------------------------
echo "  [3/3] tracing/log macros do not interpolate envelopes, payloads, or secrets"
# A macro call can legitimately span multiple lines (this codebase's
# house style wraps long calls), which is beyond what a line-oriented
# sed/grep pass can reliably parse without either missing multi-line
# calls or mistaking a word inside the human-readable message string
# (e.g. "...payload was not valid JSON...") for a real interpolation.
# _no_secrets_log_scan.pl does the same job grep/sed would (regex
# matching, no crate compiled, no cargo invoked) but with balanced-paren
# call extraction and string-literal stripping that plain grep cannot
# express. It is pure static analysis, just not line-oriented.
LOG_SCANNER="$(dirname "${BASH_SOURCE[0]}")/_no_secrets_log_scan.pl"
LOG_FILES=$(find crates -name '*.rs' 2>/dev/null | grep -v '/target/')
LOG_CALL_COUNT=$(grep -rlE '(tracing::(trace|debug|info|warn|error)|log::(trace|debug|info|warn|error))!\s*\(' \
    --include=*.rs crates/ 2>/dev/null | grep -v '/target/' | wc -l | tr -d ' ')

BAD_INTERP=""
for f in $LOG_FILES; do
    out=$(perl "$LOG_SCANNER" "$f") || true
    if [ -n "$out" ]; then
        BAD_INTERP="${BAD_INTERP}${out}"$'\n'
    fi
done

if [ -z "$BAD_INTERP" ]; then
    echo "    ok ($LOG_CALL_COUNT file(s) with log calls scanned)"
else
    echo "    [FAIL] log/tracing call interpolates a whole envelope/payload/secret:"
    echo "$BAD_INTERP" | sed 's/^/      /'
    FAILED=1
fi

echo ""
if [ $FAILED -eq 0 ]; then
    echo "[PASS] No secrets reach events, logs, or serialized output"
    exit 0
else
    echo "[FAIL] Secret-leak violation found (Verification Scripts §11; Part II Ch.1 §1.17.1)"
    echo "       AuthorityProof and credential-shaped fields must never be serialized"
    echo "       into a domain event or written into a log line. Redact or drop the"
    echo "       field before it reaches the event payload or the tracing call."
    exit 1
fi
