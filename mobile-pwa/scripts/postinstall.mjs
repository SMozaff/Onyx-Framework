/* MIGRATION_PLAN Phase 2.2 "postinstall (generate service worker)".
 *
 * Renders public/sw.js from scripts/sw.template.js, stamping the app
 * version into the cache name so a deploy invalidates outdated caches.
 * The template below is the whole service worker: precaching is
 * delegated to the built asset hashes (Vite), and this worker only
 * (a) handles a runtime cache for observer-content requests and
 * (b) listens for `push`/`notificationclick` — the Phase 3.2 Web Push
 * event handlers are wired here because a service worker is the browser
 * surface that receives push even when the tab is closed.
 *
 * Push registration itself stays in src/lib/pwa.ts (Phase 3.2).
 */
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = () => path.resolve(__dirname, '..');

async function main() {
  const pkg = JSON.parse(await readFile(path.join(root(), 'package.json'), 'utf8'));
  const template = await readFile(path.join(__dirname, 'sw.template.js'), 'utf8');
  const rendered = template
    .replace('__CACHE_VERSION__', pkg.version)
    .replace('__PRECACHE_URLS__', '');
  await writeFile(path.join(root(), 'public', 'sw.js'), rendered, 'utf8');
  console.log(`[postinstall] wrote public/sw.js (cache v${pkg.version})`);
}

main().catch((error) => {
  console.error('[postinstall] failed to generate service worker:', error);
  process.exit(1);
});