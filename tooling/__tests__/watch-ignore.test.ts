// @vitest-environment node
// Issue #65: the Vite dev server must not reload on files written by agents
// (coverage HTML inside .worktrees/<n>, cargo target output, ...).
import { readFileSync } from 'node:fs';
import { matchesGlob } from 'node:path';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import eslintConfig from '../../eslint.config.js';
import viteConfig from '../../vite.config';
import vitestConfig from '../../vitest.config';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..');

const ignored = (viteConfig.server?.watch?.ignored ?? []) as string[];

describe('vite_dev_server_ignores_worktrees', () => {
  it('Vite dev server does not watch worktrees, coverage or target', () => {
    expect(ignored).toEqual(expect.arrayContaining(['**/.worktrees/**', '**/coverage/**', '**/target/**']));
    expect(viteConfig.server?.port).toBe(1420);
    expect(viteConfig.server?.strictPort).toBe(true);
  });

  it('Vite watcher ignores a file written inside a worktree', () => {
    const matches = (file: string) => ignored.some((glob) => matchesGlob(file, glob));
    expect(matches('.worktrees/56/coverage/index.html')).toBe(true);
    expect(matches('target/debug/x')).toBe(true);
    expect(matches('coverage/index.html')).toBe(true);
    expect(matches('src/main.tsx')).toBe(false);
  });

  it('Other tools keep excluding worktrees', () => {
    const entries = eslintConfig as { ignores?: string[] }[];
    const eslintIgnores = entries.flatMap((entry) => entry.ignores ?? []);
    expect(eslintIgnores).toContain('.worktrees/**');

    const tsconfig = JSON.parse(readFileSync(join(root, 'tsconfig.json'), 'utf8')) as {
      exclude: string[];
    };
    expect(tsconfig.exclude).toContain('.worktrees/**');

    const prettierIgnore = readFileSync(join(root, '.prettierignore'), 'utf8').split(/\r?\n/);
    expect(prettierIgnore).toContain('.worktrees/');

    expect(vitestConfig.test?.exclude).toContain('.worktrees/**');
  });
});
