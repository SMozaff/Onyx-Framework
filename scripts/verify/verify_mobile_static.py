#!/usr/bin/env python3
"""Offline structural verifier for the ONYX Flutter v1.1 deliverable."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path
from xml.etree import ElementTree

import yaml

ROOT = Path(__file__).resolve().parents[2]
checks: list[dict[str, object]] = []


def check(name: str, condition: bool, detail: str = "") -> None:
    checks.append({"name": name, "passed": bool(condition), "detail": detail})
    if not condition:
        raise AssertionError(f"{name}: {detail}")


def strip_dart(text: str) -> str:
    out: list[str] = []
    i = 0
    state = "code"
    quote = ""
    while i < len(text):
        c = text[i]
        nxt = text[i + 1] if i + 1 < len(text) else ""
        if state == "code":
            if c == "/" and nxt == "/":
                state = "line"; out.extend("  "); i += 2; continue
            if c == "/" and nxt == "*":
                state = "block"; out.extend("  "); i += 2; continue
            if c in "'\"":
                if text[i:i + 3] == c * 3:
                    state = "triple"; quote = c; out.extend("   "); i += 3; continue
                state = "string"; quote = c; out.append(" "); i += 1; continue
            out.append(c); i += 1; continue
        if state == "line":
            if c == "\n": state = "code"; out.append("\n")
            else: out.append(" ")
            i += 1; continue
        if state == "block":
            if c == "*" and nxt == "/": state = "code"; out.extend("  "); i += 2
            else: out.append("\n" if c == "\n" else " "); i += 1
            continue
        if state == "string":
            if c == "\\": out.extend("  "); i += 2; continue
            if c == quote: state = "code"
            out.append(" "); i += 1; continue
        if state == "triple":
            if text[i:i + 3] == quote * 3: state = "code"; out.extend("   "); i += 3
            else: out.append("\n" if c == "\n" else " "); i += 1
    check("dart lexical state closed", state in {"code", "line"}, state)
    return "".join(out)


required = [
    "mobile/pubspec.yaml", "mobile/lib/main.dart", "mobile/lib/bridge/bridge.dart",
    "mobile/lib/ui/app.dart", "mobile/lib/ui/screens/dashboard.dart",
    "mobile/lib/ui/screens/missions.dart", "mobile/lib/ui/screens/mission_detail.dart",
    "mobile/lib/ui/screens/tasks.dart", "mobile/lib/ui/screens/task_detail.dart",
    "mobile/lib/ui/screens/notifications.dart", "mobile/lib/ui/screens/approvals.dart",
    "mobile/lib/ui/screens/settings.dart", "mobile/lib/ui/widgets/sync_status.dart",
    "mobile/lib/ui/widgets/conflict_dialog.dart",
    "mobile/lib/background/android/workmanager_service.dart",
    "mobile/lib/background/ios/background_service.dart",
    "mobile/android/app/src/main/java/com/onyx/MainActivity.kt",
    "mobile/android/app/src/main/kotlin/com/onyx/WorkManagerService.kt",
    "mobile/ios/Runner/AppDelegate.swift", "mobile/ios/Runner/BackgroundService.swift",
    "mobile/test/bridge_test.dart", "mobile/test/integration/p2p_sync_test.dart",
]
for relative in required:
    check(f"file exists: {relative}", (ROOT / relative).is_file())

for relative in ["mobile/pubspec.yaml", "mobile/analysis_options.yaml", ".github/workflows/ci.yml", ".github/workflows/increment5-clients.yml"]:
    yaml.safe_load((ROOT / relative).read_text())
    check(f"YAML parses: {relative}", True)

for relative in ["mobile/android/app/src/main/AndroidManifest.xml", "mobile/android/app/src/main/res/values/styles.xml", "mobile/ios/Runner/Info.plist"]:
    ElementTree.parse(ROOT / relative)
    check(f"XML parses: {relative}", True)

pairs = {")": "(", "]": "[", "}": "{"}
for path in sorted((ROOT / "mobile").rglob("*.dart")):
    stripped = strip_dart(path.read_text())
    stack: list[str] = []
    for token in stripped:
        if token in "([{": stack.append(token)
        elif token in ")]}":
            check(f"balanced token in {path.relative_to(ROOT)}", bool(stack) and stack[-1] == pairs[token], token)
            stack.pop()
    check(f"all delimiters closed: {path.relative_to(ROOT)}", not stack, str(stack))
    for match in re.finditer(r"import\s+['\"]([^'\"]+)['\"]", path.read_text()):
        target = match.group(1)
        if target.startswith("."):
            check(
                f"relative import resolves: {path.relative_to(ROOT)} -> {target}",
                (path.parent / target).resolve().is_file(),
            )

bridge = (ROOT / "mobile/lib/bridge/bridge.dart").read_text()
for symbol in [
    "mobile_core_new", "mobile_core_execute_command", "mobile_core_execute_query",
    "mobile_core_list_aggregates", "mobile_core_get_sync_status",
    "mobile_core_list_conflicts", "mobile_core_resolve_conflict",
    "mobile_core_trigger_sync", "mobile_core_subscribe_events",
]:
    check(f"Dart binds {symbol}", symbol in bridge)

rust_mobile = (ROOT / "crates/mobile-core/src/ffi_mobile.rs").read_text()
for symbol in ["mobile_core_list_aggregates", "mobile_core_get_sync_status", "mobile_core_list_conflicts", "mobile_core_resolve_conflict"]:
    check(f"Rust exports {symbol}", symbol in rust_mobile)

sync_agent = (ROOT / "crates/applications/client-composition/src/sync_agent.rs").read_text()
check("SyncAgent retains conflict records", "open_conflicts: Arc<tokio::sync::RwLock<Vec<ConflictRecord>>>" in sync_agent)
check("SyncAgent resolves conflict records", "pub async fn resolve_conflict" in sync_agent)
check("background pointer export exists", "mobile_core_background_sync_registered" in (ROOT / "crates/mobile-core/src/lib.rs").read_text())

ci = (ROOT / ".github/workflows/ci.yml").read_text()
for job in ["mobile-dart:", "mobile-android:", "mobile-ios:"]:
    check(f"CI mobile job {job[:-1]}", job in ci)

p2p = (ROOT / "mobile/test/integration/p2p_sync_test.dart").read_text()
check("P2P device gate is explicit", "ONYX_MOBILE_DEVICE_TEST" in p2p)
check("P2P placeholder blocker documented", "ConnectionLost" in (ROOT / "MOBILE_STATUS.md").read_text())

for script in ["mobile/tool/build_rust_android.sh", "mobile/tool/build_rust_ios.sh", "mobile/tool/ensure_platform_scaffold.sh", "scripts/verify/verify_mobile.sh"]:
    result = subprocess.run(["bash", "-n", str(ROOT / script)], capture_output=True, text=True)
    check(f"shell syntax: {script}", result.returncode == 0, result.stderr)

report = {
    "document": "ONYX Flutter Mobile v1.1 Static Verification",
    "checks_total": len(checks),
    "checks_passed": sum(1 for item in checks if item["passed"]),
    "checks": checks,
    "runtime_gates_executed": False,
    "runtime_blockers": [
        "Flutter/Dart SDK unavailable in the execution container",
        "Rust mobile targets, Android NDK, Xcode, CocoaPods, and device lab unavailable",
        "Wi-Fi Direct/BLE transport stream implementations are placeholders returning ConnectionLost",
        "pubspec.lock regeneration requires an approved networked Flutter runner",
    ],
}
output = ROOT / "docs/MOBILE_V11_STATIC_REPORT.json"
output.write_text(json.dumps(report, indent=2) + "\n")
print(f"{report['checks_passed']}/{report['checks_total']} checks passed")
