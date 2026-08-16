#!/bin/bash
# ======================================================================
# Verify every aggregate fully and non-trivially implements AggregateRoot.
#
# Source: Verification Scripts §3; Team Prompt 1 §3.9 (the AggregateRoot
# trait definition, corrected import path per ruling) and §10 (every
# bounded context's aggregate is the authoritative record of its
# lifecycle and MUST implement decide()/apply() as real state-machine
# logic, not placeholders).
#
# WHY THIS CHECK EXISTS DESPITE THE COMPILER.
# Rust's type system already refuses to compile an `impl AggregateRoot`
# block that is missing a method — that half of "trait completeness" is
# free and not worth a script. What the compiler happily accepts, and
# what this script exists to catch, is two silent failure modes that
# type-check perfectly:
#
#   1. A domain crate that should own an aggregate (crates/domains/*)
#      simply has no crates/domains/<crate>/src/aggregate.rs at all, or
#      the file exists but never implements AggregateRoot for anything.
#      Nothing about "the crate compiles" tells you a bounded context's
#      state machine was never wired up.
#
#   2. An aggregate DOES implement all six methods, but decide() has
#      been stubbed to unconditionally `Ok(vec![])` regardless of the
#      command it was given, or apply() has been stubbed to do nothing.
#      Both compile, both satisfy the trait, and both silently turn the
#      aggregate into a black hole: commands are accepted and produce no
#      events, or events are recorded but never change state. Every
#      caller believes the state machine works because nothing errors.
#
# Scope: only crates/domains/*/src/aggregate.rs are checked. Per Part I
# Ch.4 / Appendix A, only mission-domain and work-domain currently exist
# as domain crates that own an aggregate — synchronization-domain is a
# cross-cutting merge concern with no aggregate of its own (see
# command_guard.rs/escalation.rs, which reference AggregateRoot only as
# a generic bound, never implement it). The doc-inventoried gap that
# organization-domain, identity-domain, approval-domain, timeline-domain
# and audit-policy-domain don't exist as crates yet is tracked in
# docs/SPEC_CONFORMANCE_GAPS.md §2 — that is a "the crate is missing"
# gap, not a "the trait is incomplete" gap, and is out of scope here.
# ======================================================================

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

FAILED=0
REQUIRED_METHODS=(id version lifecycle_epoch authority_epoch decide apply)

echo "Verifying AggregateRoot trait completeness..."
echo ""

# ----------------------------------------------------------------------
# 1. Every domain crate that should own an aggregate has aggregate.rs.
#
# A domain crate is any crates/domains/*-domain directory. (crdt and
# synchronization-domain live under crates/synchronization and are not
# domain crates with an aggregate of their own — see header.)
# ----------------------------------------------------------------------
echo "  [1/3] every domain crate has crates/domains/<crate>/src/aggregate.rs"

if [ ! -d crates/domains ]; then
    echo "    [FAIL] crates/domains not found"
    FAILED=1
fi

DOMAIN_DIRS=$(find crates/domains -maxdepth 1 -mindepth 1 -type d -name '*-domain' 2>/dev/null | sort)

if [ -z "$DOMAIN_DIRS" ]; then
    echo "    [FAIL] no *-domain crates found under crates/domains"
    FAILED=1
fi

AGGREGATE_FILES=""
for dir in $DOMAIN_DIRS; do
    crate_name="$(basename "$dir")"
    agg_file="$dir/src/aggregate.rs"
    if [ ! -f "$agg_file" ]; then
        echo "    [FAIL] $crate_name has no src/aggregate.rs"
        FAILED=1
    else
        echo "    ok: $crate_name"
        AGGREGATE_FILES="$AGGREGATE_FILES $agg_file"
    fi
done

# ----------------------------------------------------------------------
# 2. Each aggregate.rs contains an `impl AggregateRoot for X` block with
#    all six required methods.
#
# Scoped to the text of the impl block itself (from the `impl
# AggregateRoot for` line to the closing brace at column 0), not the
# whole file, matching the house rule of checking the code that
# implements a contract rather than every mention of it. Comments are
# stripped first so a doc comment listing "id, version, lifecycle_epoch,
# authority_epoch, decide, apply" cannot be mistaken for the methods
# actually being defined.
# ----------------------------------------------------------------------
echo ""
echo "  [2/3] impl AggregateRoot block defines all six required methods"

for agg_file in $AGGREGATE_FILES; do
    crate_name="$(basename "$(dirname "$(dirname "$agg_file")")")"

    impl_block=$(sed -e 's://.*::' "$agg_file" \
        | awk '/^impl AggregateRoot for/{flag=1} flag{print} flag && /^}/{exit}')

    if [ -z "$impl_block" ]; then
        echo "    [FAIL] $crate_name: no top-level 'impl AggregateRoot for ...' block found"
        FAILED=1
        continue
    fi

    missing=""
    for method in "${REQUIRED_METHODS[@]}"; do
        if ! echo "$impl_block" | grep -qE "fn[[:space:]]+${method}[[:space:]]*\("; then
            missing="$missing $method"
        fi
    done

    if [ -n "$missing" ]; then
        echo "    [FAIL] $crate_name: impl AggregateRoot missing methods:$missing"
        FAILED=1
    else
        echo "    ok: $crate_name defines id/version/lifecycle_epoch/authority_epoch/decide/apply"
    fi
done

# ----------------------------------------------------------------------
# 3. decide() and apply() are not unconditional no-op stubs.
#
# A stub that compiles and satisfies the trait looks like:
#   fn decide(&self, _command: Self::Command, ...) -> ... { Ok(vec![]) }
#   fn apply(&mut self, _event: &Self::Event) {}
# Neither reads its parameter, and neither branches on it. The heuristic
# below is deliberately about *shape*, not implementation style: the
# parameter must be bound under its real name (not `_command`/`_event`,
# which is what you write when you intend never to touch it), and the
# body must contain a `match` on that parameter (optionally by reference,
# e.g. `match &command` — matching on a borrow so the value can still be
# used afterward is idiomatic and common, not a sign of a stub), since
# every ruled command/event enum in this codebase is dispatched by
# matching on it (Team Prompt 1 §10).
#
# Matching on the parameter is necessary but not sufficient: a stub can
# `match command { _ => Ok(vec![]) }` and satisfy a naive "contains
# match command" check while still producing the exact same no-op result
# for every variant it's given — the precise failure mode this check was
# written to catch (see header). So the body must also contain at least
# one arm that names a specific enum variant (a `Type::Variant` pattern
# before `=>`), not only a wildcard `_` arm. A real implementation with a
# single command variant still names and inspects that variant, so this
# does not false-positive on "small" aggregates — only on aggregates
# that never look at what they were given.
# ----------------------------------------------------------------------
echo ""
echo "  [3/3] decide()/apply() are not unconditional no-op stubs"

for agg_file in $AGGREGATE_FILES; do
    crate_name="$(basename "$(dirname "$(dirname "$agg_file")")")"
    stripped=$(sed -e 's://.*::' "$agg_file")

    # --- decide() ---
    decide_sig=$(echo "$stripped" | grep -E 'fn[[:space:]]+decide[[:space:]]*\(' | head -1)
    decide_body=$(echo "$stripped" \
        | awk '/fn[ \t]+decide[ \t]*\(/{flag=1} flag{print} flag && /^    fn apply/{exit} flag && /^}/{exit}')

    if echo "$decide_sig" | grep -qE '_command[[:space:]]*:'; then
        echo "    [FAIL] $crate_name: decide() ignores its command (_command: unused param) — looks like a stub"
        FAILED=1
    elif ! echo "$decide_body" | grep -qE 'match[[:space:]]+&?\*?command\b'; then
        echo "    [FAIL] $crate_name: decide() does not match on its command — looks like a stub that always returns the same result"
        FAILED=1
    elif [ "$(echo "$decide_body" | grep -cE '[A-Za-z_][A-Za-z0-9_]*::[A-Za-z_][A-Za-z0-9_]*[^=]*=>')" -eq 0 ]; then
        echo "    [FAIL] $crate_name: decide() matches on command but every arm is a wildcard — looks like a stub that returns the same result regardless of variant"
        FAILED=1
    else
        echo "    ok: $crate_name decide() dispatches on its command"
    fi

    # --- apply() ---
    apply_sig=$(echo "$stripped" | grep -E 'fn[[:space:]]+apply[[:space:]]*\(' | head -1)
    apply_body=$(echo "$stripped" \
        | awk '/fn[ \t]+apply[ \t]*\(/{flag=1} flag{print} flag && /^}/{exit}')
    # Body excluding the signature line itself, to check for an empty `{}`.
    apply_body_only=$(echo "$apply_body" | tail -n +2)
    non_brace_lines=$(echo "$apply_body_only" | grep -vcE '^\s*[{}]\s*$')

    if echo "$apply_sig" | grep -qE '_event[[:space:]]*:'; then
        echo "    [FAIL] $crate_name: apply() ignores its event (_event: unused param) — looks like a stub"
        FAILED=1
    elif [ "$non_brace_lines" -eq 0 ]; then
        echo "    [FAIL] $crate_name: apply() has an empty body — events are recorded but never mutate state"
        FAILED=1
    elif ! echo "$apply_body" | grep -qE 'match[[:space:]]+&?\*?event\b'; then
        echo "    [FAIL] $crate_name: apply() does not match on its event — looks like it does not apply state per event"
        FAILED=1
    elif [ "$(echo "$apply_body" | grep -cE '[A-Za-z_][A-Za-z0-9_]*::[A-Za-z_][A-Za-z0-9_]*[^=]*=>')" -eq 0 ]; then
        echo "    [FAIL] $crate_name: apply() matches on event but every arm is a wildcard — looks like a stub that never actually applies state per variant"
        FAILED=1
    else
        echo "    ok: $crate_name apply() dispatches on its event and is non-empty"
    fi
done

echo ""
if [ $FAILED -eq 0 ]; then
    echo "[PASS] All aggregates fully and non-trivially implement AggregateRoot"
    exit 0
else
    echo "[FAIL] AggregateRoot completeness violation found"
    echo "       Every domain crate under crates/domains must own an aggregate.rs"
    echo "       implementing all six AggregateRoot methods (Team Prompt 1 §3.9),"
    echo "       and decide()/apply() must be real state-machine logic, not"
    echo "       unconditional stubs (Team Prompt 1 §10)."
    exit 1
fi
