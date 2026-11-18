// @vitest-environment node
// Gate 7: coverage thresholds must reject insufficient reports (Rust and frontend).
import { spawnSync } from 'node:child_process';
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
    });
    expect(run.status).not.toBe(0);
    expect(`${run.stdout}${run.stderr}`).toMatch(/threshold/i);
  });
});
