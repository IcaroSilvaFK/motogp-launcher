// @vitest-environment node
// Gate 7: coverage thresholds must reject insufficient reports (Rust and frontend).
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { evaluateRustCoverage } from '../../scripts/check-rust-coverage.mjs';
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

  it.each([
    '.worktrees/translate/crates/domain/src/b.rs',
    'D:\\repo\\.worktrees\\translate\\crates\\domain\\src\\b.rs',
  ])('excludes %s from core and overall coverage', (filename) => {
    expect(evaluateRustCoverage(report({ 'crates/domain/src/a.rs': [90, 100], [filename]: [0, 1000] })).ok).toBe(true);
    expect(evaluateRustCoverage(report({ 'crates/domain/src/a.rs': [85, 100], [filename]: [1000, 1000] })).ok).toBe(
      false,
    );
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
    const result = evaluateRustCoverage(report({}));
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
