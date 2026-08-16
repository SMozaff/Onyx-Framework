#!/bin/bash
# ======================================================================
# Verify domain crates carry no infrastructure dependencies.
#
# Source: Part I §1.5 (dependency direction), Part II Chapter 1 §1.5.2
# (allowed dependency matrix) and §1.6 (the domain layer "contains no
# database handles, network clients, framework request objects, runtime
# globals, logging setup, or filesystem access"), Team Prompt 1 §7.1.
#
# Domain crates may depend on the kernel and on their own context only.
# A domain crate that reaches for sqlx or tokio has inverted the
# dependency direction the whole hexagonal layering rests on, and the
# breakage is silent: everything still compiles and passes tests, while
# the domain quietly stops being portable to the mobile and desktop
# runtimes that Part II Chapter 1 §1.1.1 requires it to run in.
# ======================================================================

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

FAILED=0

# Infrastructure crates a domain crate must never name. Matched against
# dependency KEYS in Cargo.toml, not the whole file, so that a crate
# legitimately mentioning one of these words in its description or in a
# comment does not trip the check.
FORBIDDEN=(
    sqlx sqlx-postgres sqlx-sqlite diesel sea-orm tokio-postgres
    tokio reqwest hyper axum actix actix-web rocket warp tonic
    tracing-subscriber opentelemetry prometheus redis
)

echo "Checking domain crates for prohibited dependencies..."
echo ""

# Part I's domain layer lives in crates/domains; synchronization-domain
# sits under crates/synchronization but is a domain crate by name and by
# the same rule, so it is checked too.
DOMAIN_MANIFESTS=$(find crates/domains crates/synchronization -maxdepth 2 -name Cargo.toml 2>/dev/null | sort)

if [ -z "$DOMAIN_MANIFESTS" ]; then
    echo "  No domain crates found under crates/domains or crates/synchronization."
    echo "  This check cannot pass vacuously — treating as failure."
    exit 1
fi

for manifest in $DOMAIN_MANIFESTS; do
    crate_dir="$(dirname "$manifest")"
    crate_name="$(basename "$crate_dir")"

    # Only crates whose name marks them as domain logic. sync-test-utils
    # and similar helpers under the same directory are not domain crates
    # and are allowed their test-time dependencies.
    case "$crate_name" in
        *-domain|crdt) ;;
        *) continue ;;
    esac

    echo "  Checking: $crate_name"

    # Extract dependency names from [dependencies] and
    # [build-dependencies] only, taking the key left of '='.
    #
    # [dev-dependencies] is deliberately NOT checked. Team Prompt 3 §6.2's
    # own example tests are written `#[tokio::test]`, so the frozen spec
    # requires tokio as a dev-dependency of the very crate whose runtime
    # dependency on tokio it forbids. Flagging dev-dependencies would fail
    # a crate for doing exactly what its Team Prompt tells it to do. What
    # matters for the dependency-direction rule is what ships, not what
    # the test harness links.
    deps=$(awk '
        /^\[dependencies\]/          { in_deps = 1; next }
        /^\[build-dependencies\]/    { in_deps = 1; next }
        /^\[/                        { in_deps = 0 }
        in_deps && /=/               { sub(/[[:space:]]*=.*/, ""); gsub(/[[:space:]]/, ""); if ($0 != "") print }
    ' "$manifest")

    for dep in $deps; do
        for bad in "${FORBIDDEN[@]}"; do
            if [ "$dep" = "$bad" ]; then
                echo "    [FAIL] prohibited dependency: $dep"
                FAILED=1
            fi
        done
    done

    # A path dependency reaching into infrastructure or transports is the
    # same violation wearing a different hat, and is not caught by the
    # name list above.
    if grep -qE 'path[[:space:]]*=[[:space:]]*"[^"]*(infrastructure|transports|bins)/' "$manifest"; then
        echo "    [FAIL] path dependency on an infrastructure/transport/binary crate"
        FAILED=1
    fi
done

echo ""
if [ $FAILED -eq 0 ]; then
    echo "[PASS] All domain crates have clean dependencies"
    exit 0
else
    echo "[FAIL] Dependency violations found"
    echo "       Domain crates may depend on the kernel and their own context only."
    echo "       Move the offending code behind an application-layer port and"
    echo "       implement it in an infrastructure crate (Part II Ch.1 §1.10)."
    exit 1
fi
