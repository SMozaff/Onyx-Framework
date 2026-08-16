import { readdir, stat } from 'node:fs/promises';
import { gzipSync } from 'node:zlib';
import { readFile } from 'node:fs/promises';
const dir = new URL('../dist/assets/', import.meta.url);
let total = 0;
for (const file of await readdir(dir)) {
  if (!file.endsWith('.js')) continue;
  const data = await readFile(new URL(file, dir));
  total += gzipSync(data).length;
}
const max = 500 * 1024;
console.log(`Initial JavaScript gzip size: ${total} bytes`);
if (total > max) process.exit(1);
