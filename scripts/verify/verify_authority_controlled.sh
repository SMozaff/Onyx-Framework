#!/bin/bash
# ======================================================================
# Verify authority-controlled fields never reach CRDT merge.
#
# Source: Part I Governing Principle 4 and AD-008 ("authority-controlled
# state is never resolved by Last-Write-Wins"), Part I §4.16.4, Part II
# §7.4.2 ("a CONCURRENT pair of operations on an authority-controlled
# field always produces a ConflictRecord, unconditionally — the
# dispatcher is not consulted, because there is no data shape that makes
# authority state safely mergeable").
#
# This is the invariant with the worst failure mode in the system. A
# violation does not crash, does not fail a test, and does not surface to
# a user: it silently accepts an unauthorized ownership or lifecycle
# change on one replica and converges every other replica onto it. By the
# time anyone notices, the audit trail records the wrong state as
# legitimate. That is why the check is a build gate and not a review
# checklist item.
# ======================================================================

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

FAILED=0
CRDT_SRC="crates/synchronization/crdt/src"

echo "Verifying authority-controlled fields bypass CRDT merge..."
echo ""

# ----------------------------------------------------------------------
# 1. The crdt crate must not know what authority is.
#
# The crate implements six data shapes (Part I §3.5). None of them takes
# authority into account, by design — AuthorityControlled is not a shape
# the dispatcher handles, it is the case that skips the dispatcher
# entirely. If the crdt crate has learned the word, something has taught
# it to merge a field it must refuse.
#
# Comments are stripped before matching so that a doc comment explaining
# the rule does not itself trip it.
# ----------------------------------------------------------------------
echo "  [1/3] crdt crate does not reference authority state"
if [ ! -d "$CRDT_SRC" ]; then
    echo "    [FAIL] $CRDT_SRC not found"
    FAILED=1
else
    hits=$(find "$CRDT_SRC" -name '*.rs' -print0 \
        | xargs -0 sed -e 's://.*::' \
        | grep -inE 'authority_epoch|AuthorityControlled|AuthorityProof|authority_controlled' \
        || true)
    if [ -n "$hits" ]; then
        echo "    [FAIL] crdt crate references authority-controlled state:"
        echo "$hits" | sed 's/^/      /'
        FAILED=1
    else
        echo "    ok"
    fi
fi

# ----------------------------------------------------------------------
# 2. The AuthorityControlled shape must not be dispatched to a merge.
#
# Part II §7.4.2 is explicit that this arm creates a ConflictRecord. The
# failure this catches is someone adding a merge call to that match arm
# to "handle" a case they saw falling through.
# ----------------------------------------------------------------------
echo "  [2/3] AuthorityControlled dispatch arm creates a conflict, not a merge"
#
# Scoped to the dispatcher itself. Part II §7.4.1 makes MergeStrategy a
# single dispatcher, so there is exactly one place this arm can exist and
# exactly one file worth checking. Scanning every file that merely
# contains the word instead flags the enum's own definition
# (field_metadata.rs) and any doc comment describing the rule
# (session.rs) — both of which are correct code that has no Conflict
# outcome because neither is the dispatcher.
DISPATCHER="crates/synchronization/synchronization-domain/src/merge_strategy.rs"
if [ ! -f "$DISPATCHER" ]; then
    echo "    [FAIL] dispatcher not found at $DISPATCHER (Part II §7.4.1)"
    FAILED=1
else
    # Comments stripped so a doc comment quoting the rule is not mistaken
    # for the code implementing it.
    arm=$(sed -e 's://.*::' "$DISPATCHER" | grep -A10 'AuthorityControlled' || true)
    if [ -z "$arm" ]; then
        echo "    [FAIL] $DISPATCHER has no AuthorityControlled arm in code"
        FAILED=1
    else
        if echo "$arm" | grep -qE '\.merge\(|merge_field\(|MergeOutcome::Applied'; then
            echo "    [FAIL] AuthorityControlled arm reaches a merge or Applied outcome"
            FAILED=1
        elif ! echo "$arm" | grep -qE 'MergeOutcome::Conflict'; then
            echo "    [FAIL] AuthorityControlled arm does not produce MergeOutcome::Conflict"
            FAILED=1
        else
            echo "    ok"
        fi
    fi
fi

# ----------------------------------------------------------------------
# 3. Authority-controlled fields must not be declared CRDT-eligible.
#
# FieldMetadata carries both flags (Part II §3.10). A field marked both
# is a contradiction the type system does not prevent, so it is checked
# here: is_mergeable() already excludes it at runtime, but a field
# declared both ways means whoever wrote it believed it was mergeable.
# ----------------------------------------------------------------------
echo "  [3/3] no field declared both authority-controlled and CRDT-eligible"
both=$(grep -rn --include=*.rs -B4 -A4 'is_authority_controlled:[[:space:]]*true' crates/ 2>/dev/null \
    | grep 'is_crdt_eligible:[[:space:]]*true' || true)
if [ -n "$both" ]; then
    echo "    [FAIL] field declared authority-controlled AND crdt-eligible:"
    echo "$both" | sed 's/^/      /'
    FAILED=1
else
    echo "    ok"
fi

echo ""
if [ $FAILED -eq 0 ]; then
    echo "[PASS] Authority-controlled fields correctly bypass CRDT merge"
    exit 0
else
    echo "[FAIL] AD-008 violation: authority state is reachable by CRDT merge"
    echo "       Authority-controlled fields MUST create a ConflictRecord on any"
    echo "       CONCURRENT pair (Part II §7.4.2). There is no data shape that"
    echo "       makes authority state safely mergeable."
    exit 1
fi
