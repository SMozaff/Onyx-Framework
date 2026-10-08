import { mkdir, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';

const output = process.argv[2];
const component = process.argv[3] ?? 'unspecified-ui-component';
if (!output) {
  console.error('Usage: node scripts/collect-ui-evidence.mjs <output-path> <component>');
  process.exit(2);
}

const commands = (process.env.UI_EVIDENCE_COMMANDS ?? '')
  .split('\n')
  .map((command) => command.trim())
  .filter(Boolean);

const manifest = {
  schema_version: '1.0',
  generated_at: new Date().toISOString(),
  commit_sha: process.env.GITHUB_SHA ?? process.env.ONYX_COMMIT_SHA ?? 'local-uncommitted',
  component,
  status: process.env.UI_EVIDENCE_STATUS ?? 'passed',
  runner: {
    os: process.platform,
    arch: process.arch,
    github_run_id: process.env.GITHUB_RUN_ID ?? null,
    github_run_attempt: process.env.GITHUB_RUN_ATTEMPT ?? null,
  },
  commands,
  artifact_contract: {
    purpose: 'UI smoke-evidence provenance only',
    runtime_claim: 'This manifest records the listed checks. It does not assert a native-window launch unless a platform-specific runtime command is included in commands.',
  },
};

await mkdir(dirname(output), { recursive: true });
await writeFile(output, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
console.log(`Wrote UI evidence manifest: ${output}`);
