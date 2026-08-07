import { readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

function walk(directory: string): string[] {
  return readdirSync(directory).flatMap((name) => {
    const path = join(directory, name);
    return statSync(path).isDirectory() ? walk(path) : [path];
  });
}

describe('v1 feature scope audit', () => {
  it('contains no excluded feature implementation', () => {
    const root = join(dirname(fileURLToPath(import.meta.url)), '../../src');
    const excluded = [
      /class\s+OfflineQueue/i,
      /function\s+uploadFile/i,
      /createBlueprint/i,
      /MeetingChat/i,
      /localStorage\.setItem/i,
    ];
    const violations: string[] = [];
    for (const file of walk(root).filter((path) => /\.(ts|tsx)$/.test(path))) {
      const content = readFileSync(file, 'utf8');
      for (const pattern of excluded) if (pattern.test(content)) violations.push(`${file}: ${pattern}`);
    }
    expect(violations).toEqual([]);
  });

  it('uses sessionStorage only for authentication material', () => {
    const root = join(dirname(fileURLToPath(import.meta.url)), '../../src');
    const writes = walk(root).filter((path) => /\.(ts|tsx)$/.test(path)).flatMap((file) => {
      const content = readFileSync(file, 'utf8');
      return content.includes('sessionStorage.setItem') ? [file] : [];
    });
    expect(writes).toHaveLength(1);
    expect(writes[0]).toContain('utils/auth.ts');
  });
});
