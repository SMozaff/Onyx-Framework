#!/usr/bin/env python3
"""Regenerate the current ONYX mobile source-inventory report.

This is an inventory/static path check only; it does not claim Android, browser,
device, or end-to-end tests passed. Missing required files are failures and make
the process exit non-zero.
"""
from __future__ import annotations
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REPORT = ROOT / "docs" / "MOBILE_V11_STATIC_REPORT.json"
REQUIRED = [
    "mobile-android/settings.gradle.kts",
    "mobile-android/build.gradle.kts",
    "mobile-android/app/build.gradle.kts",
    "mobile-android/app/src/main/AndroidManifest.xml",
    "mobile-android/app/src/main/kotlin/com/onyx/MainActivity.kt",
    "mobile-android/app/src/main/kotlin/com/onyx/bridge/MobileCoreBridge.kt",
    "mobile-android/app/src/main/kotlin/com/onyx/net/AuthApi.kt",
    "mobile-android/app/src/main/kotlin/com/onyx/ui/LoginScreen.kt",
    "mobile-android/app/src/main/kotlin/com/onyx/session/OnyxSessionViewModel.kt",
    "mobile-android/app/src/main/kotlin/com/onyx/security/SecureTokenStore.kt",
    "mobile-android/app/src/test/kotlin/com/onyx/model/ApprovalsFilterTest.kt",
    "mobile-android/app/src/androidTest/kotlin/com/onyx/MobileCoreRoundTripTest.kt",
    "mobile-pwa/package.json",
    "mobile-pwa/src/main.tsx",
    "mobile-pwa/src/api/client.ts",
    "mobile-pwa/src/pages/Login/index.tsx",
    "mobile-pwa/src/stores/authStore.ts",
    "mobile-pwa/tests/component/Login.test.tsx",
    "mobile-pwa/public/manifest.webmanifest",
    "mobile-pwa/public/sw.js",
]
checks = []
for rel in REQUIRED:
    exists = (ROOT / rel).is_file()
    checks.append({
        "name": f"required current mobile file exists: {rel}",
        "passed": exists,
        "detail": "" if exists else "Required file is missing from this checkout.",
    })
try:
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout.strip()
except (OSError, subprocess.CalledProcessError):
    commit = "unknown"
report = {
    "document": "ONYX current mobile source inventory",
    "scope": "File-existence checks only; not a build, runtime, device, or E2E test result.",
    "source_commit": commit,
    "generated_at_utc": datetime.now(timezone.utc).isoformat(),
    "checks_total": len(checks),
    "checks_passed": sum(item["passed"] for item in checks),
    "checks_failed": sum(not item["passed"] for item in checks),
    "checks": checks,
}
REPORT.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(f"Mobile source inventory: {report['checks_passed']}/{report['checks_total']} checks passed")
if report["checks_failed"]:
    for item in checks:
        if not item["passed"]:
            print(f"FAIL: {item['name']}", file=sys.stderr)
    raise SystemExit(1)
