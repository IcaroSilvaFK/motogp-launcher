// @vitest-environment node
// Gate 7: coverage thresholds must reject insufficient reports (Rust and frontend).
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { crateHasSource, evaluateRustCoverage } from '../../scripts/check-rust-coverage.mjs';
import thresholds from '../coverage-thresholds.json';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..', '..');

/** Builds a report in the format of `cargo llvm-cov --json --summary-only`. */
function report(files: Record<string, [covered: number, count: number]>) {
  return {
    data: [
      {
        files: Object.entries(files).map(([filename, [covered, count]]) => ({
          filename,
          summary: { lines: { count, covered, percent: count ? (covered / count) * 100 : 0 } },
        })),
      },
    ],
  };
}

describe('coverage_thresholds_reject_insufficient_coverage', () => {
  it('declares 90% for the core and 80% overall', () => {
    expect(thresholds).toEqual({ core: 90, overall: 80 });
  });

  it('rejects application below 90% even when overall coverage is above 80%', () => {
    const result = evaluateRustCoverage(
      report({
        'D:/x/crates/domain/src/a.rs': [100, 100],
        'D:/x/crates/application/src/b.rs': [85, 100],
        'D:/x/crates/infrastructure/src/c.rs': [300, 300],
      }),
    );
    expect(result.ok).toBe(false);
    expect(result.failures.join('\n')).toMatch(/motogp-application.*85/);
  });

  it('rejects overall coverage below 80%', () => {
    const result = evaluateRustCoverage(
      report({
        'D:/x/crates/domain/src/a.rs': [95, 100],
        'D:/x/crates/infrastructure/src/c.rs': [10, 100],
      }),
    );
    expect(result.ok).toBe(false);
    expect(result.failures.join('\n')).toMatch(/overall/);
  });

  it('passes when all thresholds are met', () => {
    const result = evaluateRustCoverage(
      report({
        'D:/x/crates/domain/src/a.rs': [90, 100],
        'D:/x/crates/application/src/b.rs': [95, 100],
        'D:/x/crates/infrastructure/src/c.rs': [80, 100],
      }),
    );
    expect(result).toMatchObject({ ok: true, failures: [] });
  });

  it.each(['crates/application/src/b.rs', 'C:\\work\\crates\\application\\src\\b.rs'])(
    'enforces the core threshold for %s',
    (filename) => {
      const result = evaluateRustCoverage(
        report({ [filename]: [85, 100], 'crates/infrastructure/src/c.rs': [100, 100] }),
      );
      expect(result.ok).toBe(false);
      expect(result.failures.join('\n')).toMatch(/motogp-application/);
    },
  );

  it.each([
    { data: [] },
    report({ 'crates/domain/src/a.rs': [NaN, 100] }),
    report({ 'crates/domain/src/a.rs': [101, 100] }),
    report({ 'crates/domain/src/a.rs': [-1, 100] }),
    report({ 'crates/domain/src/a.rs': [0, -1] }),
  ])('rejects malformed coverage data: %j', (input) => {
    expect(() => evaluateRustCoverage(input)).toThrow(/invalid.*coverage/i);
  });

  it.each(['.worktrees/translate/crates/domain/src/b.rs', '/repo/.worktrees/translate/crates/domain/src/b.rs'])(
    'excludes nested worktree file %s from core and overall coverage',
    (filename) => {
      const options = { root: '/repo', hasSource: () => false };
      expect(
        evaluateRustCoverage(report({ 'crates/domain/src/a.rs': [90, 100], [filename]: [0, 1000] }), options).ok,
      ).toBe(true);
      expect(
        evaluateRustCoverage(report({ 'crates/domain/src/a.rs': [85, 100], [filename]: [1000, 1000] }), options).ok,
      ).toBe(false);
    },
  );

  it.each([
    [
      '/repo/.worktrees/7',
      '/repo/.worktrees/7/crates/domain/src/a.rs',
      '/repo/.worktrees/7/.worktrees/old/crates/domain/src/b.rs',
    ],
    [
      'D:\\repo\\.worktrees\\7',
      'D:\\repo\\.worktrees\\7\\crates\\domain\\src\\a.rs',
      'D:\\repo\\.worktrees\\7\\.worktrees\\old\\crates\\domain\\src\\b.rs',
    ],
  ])('counts files of the checkout itself when it lives inside .worktrees (root %s)', (cwd, own, nested) => {
    const options = { root: cwd, hasSource: () => false };
    const low = evaluateRustCoverage(report({ [own]: [10, 100] }), options);
    expect(low.ok).toBe(false);
    expect(low.failures.join('\n')).toMatch(/motogp-domain/);
    // a nested worktree under the active root is still excluded
    const high = evaluateRustCoverage(report({ [own]: [95, 100], [nested]: [0, 1000] }), options);
    expect(high.ok).toBe(true);
  });

  it('fails when a core crate has source code but zero measurable lines', () => {
    const result = evaluateRustCoverage(report({}), {
      root: '/repo',
      hasSource: (crate: { name: string }) => crate.name === 'motogp-domain',
    });
    expect(result.ok).toBe(false);
    expect(result.failures.join('\n')).toMatch(/motogp-domain.*no measurable lines/);
  });

  it('the CLI counts the real report files when run from inside a worktree', () => {
    const directory = mkdtempSync(join(tmpdir(), 'motogp-coverage-'));
    try {
      const checkout = join(directory, '.worktrees', '9');
      mkdirSync(join(checkout, 'crates', 'domain', 'src'), { recursive: true });
      const path = join(directory, 'report.json');
      writeFileSync(path, JSON.stringify(report({ [join(checkout, 'crates', 'domain', 'src', 'a.rs')]: [10, 100] })));
      const run = spawnSync(process.execPath, [join(root, 'scripts/check-rust-coverage.mjs'), path], {
        encoding: 'utf8',
        timeout: 10_000,
        cwd: checkout,
      });
      expect(run.status).toBe(1);
      expect(run.stderr).toMatch(/FAIL.*motogp-domain/);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  it('the CLI exits unsuccessfully for insufficient coverage', () => {
    const directory = mkdtempSync(join(tmpdir(), 'motogp-coverage-'));
    try {
      const path = join(directory, 'report.json');
      writeFileSync(path, JSON.stringify(report({ 'crates/domain/src/a.rs': [10, 100] })));
      const run = spawnSync(process.execPath, [join(root, 'scripts/check-rust-coverage.mjs'), path], {
        encoding: 'utf8',
        timeout: 10_000,
      });
      expect(run.error).toBeUndefined();
      expect(run.status).toBe(1);
      expect(run.stderr).toMatch(/FAIL.*motogp-domain/);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });

  it('explicitly reports a core crate with no measurable lines instead of silently approving it', () => {
    const result = evaluateRustCoverage(report({}), { root: '/repo', hasSource: () => false });
    expect(result.ok).toBe(true);
    expect(result.notes.join('\n')).toMatch(/motogp-domain.*no measurable lines/);
    expect(result.notes.join('\n')).toMatch(/motogp-application.*no measurable lines/);
  });

  it('frontend: vitest fails the gate when the core is below 90%', () => {
    const fixture = join(root, 'tests', 'fixtures', 'coverage-low');
    const vitest = join(root, 'node_modules', 'vitest', 'vitest.mjs');
    const run = spawnSync(process.execPath, [vitest, 'run', '--root', fixture, '--coverage'], {
      encoding: 'utf8',
      timeout: 20_000,
    });
    expect(run.error).toBeUndefined();
    expect(run.status).toBe(1);
    expect(`${run.stdout}${run.stderr}`).toContain('"src/domain/**" threshold (90%)');
  }, 30_000);
});

describe('coverage_windows_path_canonicalization', () => {
  const bs = (path: string) => path.split('/').join(String.fromCharCode(92));
  const win = (path: string) => ({ root: bs(path), hasSource: () => false });
  it.each([
    ['//?/D:/repo', '//?/D:/repo/crates/domain/src/a.rs', '//?/D:/repo/.worktrees/x/crates/domain/src/b.rs'],
    ['/??/D:/repo', '/??/D:/repo/crates/domain/src/a.rs', '/??/D:/repo/.worktrees/x/crates/domain/src/b.rs'],
    [
      '//?/UNC/srv/share/repo',
      '//?/UNC/srv/share/repo/crates/domain/src/a.rs',
      '//?/UNC/srv/share/repo/.worktrees/x/crates/domain/src/b.rs',
    ],
    ['D:/Repo', 'd:/repo/crates/domain/src/a.rs', 'D:/REPO/.WORKTREES/x/crates/domain/src/b.rs'],
    ['//?/d:/repo', 'D:/REPO/crates/domain/src/a.rs', '//?/D:/Repo/.WorkTrees/x/crates/domain/src/b.rs'],
  ])('root %s: strips prefixes/case and excludes nested worktrees', (rootPath, own0, nested0) => {
    const [own, nested] = [bs(own0), bs(nested0)];
    const options = win(rootPath);
    expect(evaluateRustCoverage(report({ [own]: [90, 100], [nested]: [0, 1000] }), options).ok).toBe(true);
    // an 85% core file plus covered nested-worktree files must still fail
    const result = evaluateRustCoverage(report({ [own]: [85, 100], [nested]: [1000, 1000] }), options);
    expect(result.ok).toBe(false);
    expect(result.failures.join('\n')).toMatch(/motogp-domain.*85/);
  });

  it('keeps POSIX paths case-sensitive', () => {
    const options = { root: '/repo', hasSource: () => false };
    const result = evaluateRustCoverage(
      report({ '/repo/crates/domain/src/a.rs': [85, 100], '/repo/.WORKTREES/x/crates/domain/src/b.rs': [100, 100] }),
      options,
    );
    // `.WORKTREES` is a different directory on POSIX, so it is NOT nested and its coverage counts
    expect(result.ok).toBe(true);
  });
});

describe('coverage_source_detection', () => {
  const crate = { name: 'motogp-domain', dir: 'crates/domain/src' };
  const withCrate = (files: Record<string, string>, check: (dir: string) => void) => {
    const directory = mkdtempSync(join(tmpdir(), 'motogp-src-'));
    try {
      for (const [name, content] of Object.entries(files)) {
        const full = join(directory, crate.dir, name);
        mkdirSync(dirname(full), { recursive: true });
        writeFileSync(full, content);
      }
      check(directory);
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  };

  it.each([
    ['empty crate', {}],
    ['only comments', { 'lib.rs': '// c\n\n' }],
    ['inner doc block comment', { 'lib.rs': '/*! crate docs\n pub fn x() {} */\n' }],
    ['nested block comment', { 'lib.rs': '/* a /* b */ fn x() {} */\n' }],
    ['empty cfg(test) module', { 'lib.rs': '#[cfg(test)]\nmod tests {}\n' }],
    ['cfg(test) module with code', { 'lib.rs': '#[cfg(test)]\nmod tests { fn t() { let _ = "}"; } }\n' }],
    [
      'declaration-only mod with comment-only child',
      { 'lib.rs': 'pub mod child;\nmod other;\n', 'child.rs': '//! docs\n', 'other.rs': '/* nothing */' },
    ],
  ])('has no production code: %s', (_name, files) => {
    withCrate(files, (dir) => expect(crateHasSource(dir, crate)).toBe(false));
  });

  it.each([
    ['real fn', { 'lib.rs': '/*! docs */\npub fn x() {}\n' }],
    ['fn after cfg(test) module', { 'lib.rs': '#[cfg(test)]\nmod tests {}\nfn real() {}\n' }],
    ['child module with code', { 'lib.rs': 'mod child;\n', 'child.rs': 'pub fn y() {}\n' }],
  ])('has production code: %s', (_name, files) => {
    withCrate(files, (dir) => expect(crateHasSource(dir, crate)).toBe(true));
  });

  it('fails the gate for a crate with a real fn missing from the report', () => {
    withCrate({ 'lib.rs': 'pub fn x() {}\n' }, (dir) => {
      const result = evaluateRustCoverage(report({}), { root: dir });
      expect(result.ok).toBe(false);
      expect(result.failures.join('\n')).toMatch(/motogp-domain.*no measurable lines/);
    });
  });

  it('passes the gate for a comment-only crate missing from the report', () => {
    withCrate({ 'lib.rs': '/*! docs */\n#[cfg(test)]\nmod tests {}\n' }, (dir) => {
      expect(evaluateRustCoverage(report({}), { root: dir }).ok).toBe(true);
    });
  });
});
