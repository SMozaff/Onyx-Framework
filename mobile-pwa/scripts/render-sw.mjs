/* MIGRATION_PLAN Phase 5.1 "manual generation to produce sw.js and
 * pre-cache the shell".
 *
 * Runs after `vite build`: reads the Vite build manifest
 * (build.manifest:true → dist/.vite/manifest.json), collects every
 * hash-named asset reachable from the entry, and renders dist/sw.js with
 * that precache list stamped into the template. The production worker is
 * therefore install-time offline-first: cache.add() the shell at install
 * so a later offline launch never has to hit the network.
 *
 * Dev keeps its own public/sw.js (rendered by scripts/postinstall.mjs
 * with an empty precache list), where the runtime cache learns everything
 * after the first visit.
 */
import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = () => path.resolve(__dirname, '..');

/** Bare place-holder inside the template's precache array literal. */
const PRECACHE_SLOT = '__PRECACHE_URLS__';

/** Collects the entry's JS/CSS/assets plus its transitive imports. */
function collectUrls(manifest, entryName, seen, urls) {
  const entry = manifest[entryName];
  if (!entry || seen.has(entryName)) return;
  seen.add(entryName);
  if (entry.file) urls.add(`/${entry.file}`);
  for (const css of entry.css ?? []) urls.add(`/${css}`);
  for (const asset of entry.assets ?? []) urls.add(`/${asset}`);
  for (const imp of entry.imports ?? []) collectUrls(manifest, imp, seen, urls);
}

async function main() {
  const pkg = JSON.parse(await readFile(path.join(root(), 'package.json'), 'utf8'));
  const template = await readFile(path.join(__dirname, 'sw.template.js'), 'utf8');

  let manifest;
  try {
    manifest = JSON.parse(
      await readFile(path.join(root(), 'dist', '.vite', 'manifest.json'), 'utf8'),
    );
  } catch (error) {
    throw new Error(`vite manifest not found at dist/.vite/manifest.json — run vite build first (${error.message})`);
  }

  const urls = new Set(['/', '/manifest.webmanifest', '/icons/icon.svg']);
  collectUrls(manifest, 'index.html', new Set(), urls);
  const precache = [...urls].sort();
  if (precache.length < 3) {
    throw new Error('render-sw: expected at least the document, manifest and icon in the precache list');
  }

  const rendered = template
    .replace('__CACHE_VERSION__', pkg.version)
    .replace(PRECACHE_SLOT, JSON.stringify(precache).slice(1, -1));
  await writeFile(path.join(root(), 'dist', 'sw.js'), rendered, 'utf8');
  console.log(`[render-sw] wrote dist/sw.js — precaching ${precache.length} URLs (cache v${pkg.version})`);
}

main().catch((error) => {
  console.error('[render-sw] failed to render service worker:', error);
  process.exit(1);
});