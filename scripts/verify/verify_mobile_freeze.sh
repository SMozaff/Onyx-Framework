#!/usr/bin/env bash
# Enforces the mobile/ freeze (ONYX-MOB-00 §8, M0 / DECISIONS.md H10.M0):
# the Flutter Android app is a Frozen Reference Implementation. Ordinary
# new product development in mobile/lib/ is no longer allowed -- only
# security fixes and critical defects, and those must say so explicitly
# by touching mobile/FROZEN_EXCEPTION.md in the same change.
#
# This is a real process guard, not a verbal policy: a change that
# touches mobile/lib/ without also touching mobile/FROZEN_EXCEPTION.md
# fails CI. It deliberately does not gate mobile/test/, mobile/android/,
# mobile/ios/, or any other mobile/ subdirectory -- only mobile/lib/ is
# the frozen application code the manifesto is actually talking about;
# platform scaffold and test changes needed to keep the frozen app
# building/passing on newer toolchains are not "new product development"
# and would otherwise make this guard actively harmful.
set -euo pipefail

base_ref="${1:-origin/main}"

if ! git rev-parse --verify "$base_ref" >/dev/null 2>&1; then
    echo "verify_mobile_freeze: base ref '$base_ref' not found; skipping (not enough git history to compare)" >&2
    exit 0
fi

merge_base="$(git merge-base "$base_ref" HEAD)"
changed="$(git diff --name-only "$merge_base" HEAD)"

if ! grep -q '^mobile/lib/' <<<"$changed"; then
    echo "verify_mobile_freeze: no mobile/lib/ changes in this diff -- OK"
    exit 0
fi

echo "verify_mobile_freeze: this diff touches mobile/lib/ (the frozen Flutter reference):"
grep '^mobile/lib/' <<<"$changed" | sed 's/^/  /'

if grep -qx 'mobile/FROZEN_EXCEPTION.md' <<<"$changed"; then
    echo "verify_mobile_freeze: mobile/FROZEN_EXCEPTION.md was touched in the same diff -- exception acknowledged, OK"
    exit 0
fi

cat >&2 <<'EOF'

verify_mobile_freeze: BLOCKED.

mobile/lib/ is a Frozen Reference Implementation (ONYX-MOB-00 §8): no
ordinary new product development is permitted there while the Kotlin
Android rewrite is underway. Only security fixes and critical defects
may still land, and every such change must edit mobile/FROZEN_EXCEPTION.md
in the same commit/PR, stating what was fixed and why it qualifies as a
security fix or critical defect (not a feature, refactor, or
"improvement").

If this change is a real security fix or critical defect: add that
justification to mobile/FROZEN_EXCEPTION.md and push again.

If this change is ordinary feature work: it does not belong in
mobile/lib/ -- the Kotlin Android rewrite (mobile-android/) or the PWA
(mobile-pwa/) are the active development targets now.
EOF
exit 1
