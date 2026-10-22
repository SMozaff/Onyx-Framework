#!/usr/bin/env node
/**
 * Cross-platform entry point for icon generation/validation.
 * Requires Python 3 and Pillow; honors ONYX_PYTHON when a non-default interpreter is needed.
 */
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const configured = process.env.ONYX_PYTHON;
const candidates = configured
  ? [configured]
  : process.platform === "win32"
    ? ["python", "py"]
    : ["python3", "python"];

let selected = null;
for (const executable of candidates) {
  const probeArgs = executable === "py" ? ["-3", "--version"] : ["--version"];
  const probe = spawnSync(executable, probeArgs, {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  });
  if (!probe.error && probe.status === 0) {
    selected = { executable, prefix: executable === "py" ? ["-3"] : [] };
    break;
  }
}

if (!selected) {
  console.error(
    "ONYX icon tooling needs Python 3. Set ONYX_PYTHON or install one of: " + candidates.join(", ") + ".",
  );
  process.exit(1);
}

for (const script of [
  "scripts/branding/generate-native-icons.py",
  "scripts/branding/validate-icons.py",
]) {
  const result = spawnSync(
    selected.executable,
    [...selected.prefix, script],
    { cwd: root, stdio: "inherit", windowsHide: true },
  );
  if (result.error) {
    console.error("Could not run " + script + ": " + result.error.message);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}
