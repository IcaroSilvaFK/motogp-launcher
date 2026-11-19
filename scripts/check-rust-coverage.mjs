// Rust coverage gate (gate 7): reads the JSON of `cargo llvm-cov --json --summary-only`
// and enforces the limits of tooling/coverage-thresholds.json:
//   - >= core% of lines in EACH core crate (motogp-domain, motogp-application)
//   - >= overall% of lines across the workspace
// A core crate without measurable lines is reported explicitly (never a silent pass).
//
// Usage: node scripts/check-rust-coverage.mjs <report.json>
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const thresholds = JSON.parse(readFileSync(join(here, '..', 'tooling', 'coverage-thresholds.json'), 'utf8'));

/** Core crates and the source folder that identifies each of them in the report. */
export const CORE_CRATES = [
  { name: 'motogp-domain', dir: 'crates/domain/src', marker: '/crates/domain/src/' },
  { name: 'motogp-application', dir: 'crates/application/src', marker: '/crates/application/src/' },
];

const sumLines = (files) =>
  files.reduce(
    (acc, f) => ({ count: acc.count + f.summary.lines.count, covered: acc.covered + f.summary.lines.covered }),
    { count: 0, covered: 0 },
  );

const normalize = (path) => path.replace(/\\/g, '/');

/** Path of `file` relative to `root` (both slash-normalized); unchanged when it is not under root. */
function relativeTo(root, file) {
  const base = normalize(root).replace(/\/+$/, '');
  const f = normalize(file);
  return f.toLowerCase().startsWith(`${base.toLowerCase()}/`) ? f.slice(base.length + 1) : f;
}

/** True when the crate has non-test Rust code (anything beyond blank lines and comments). */
function crateHasSource(root, crate) {
  const walk = (dir) =>
    readdirSync(dir, { withFileTypes: true }).some((entry) => {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) return entry.name !== 'tests' && walk(full);
      if (!entry.name.endsWith('.rs') || /^tests?\.rs$/.test(entry.name)) return false;
      return readFileSync(full, 'utf8')
        .split('\n')
        .some((line) => line.trim() !== '' && !line.trim().startsWith('//'));
    });
  const dir = join(root, crate.dir);
  return existsSync(dir) && walk(dir);
}

const pct = (covered, count) => (count === 0 ? 100 : (covered / count) * 100);

/**
 * @param {{ data: { files: { filename: string, summary: { lines: { count: number, covered: number } } }[] }[] }} report
 * @param {{ root?: string, hasSource?: (crate: { name: string, dir: string }) => boolean }} [options]
 *   root: active workspace root (default cwd). Only worktrees NESTED under it are excluded.
 * @returns {{ ok: boolean, failures: string[], notes: string[] }}
 */
export function evaluateRustCoverage(report, options = {}) {
  const root = options.root ?? process.cwd();
  const hasSource = options.hasSource ?? ((crate) => crateHasSource(root, crate));
  if (
    !Array.isArray(report?.data) ||
    report.data.length === 0 ||
    report.data.some((entry) => !Array.isArray(entry?.files))
  ) {
    throw new Error('Invalid Rust coverage report');
  }
  const files = report.data
    .flatMap((d) => d.files)
    .map((file) => {
      const lines = file?.summary?.lines;
      if (
        typeof file?.filename !== 'string' ||
        !file.filename ||
        !Number.isSafeInteger(lines?.count) ||
        !Number.isSafeInteger(lines?.covered) ||
        lines.count < 0 ||
        lines.covered < 0 ||
        lines.covered > lines.count
      ) {
        throw new Error('Invalid Rust coverage file or line counters');
      }
      return { ...file, filename: `/${relativeTo(root, file.filename).replace(/^\/+/, '')}` };
    })
    .filter((file) => !file.filename.startsWith('/.worktrees/'));
  const failures = [];
  const notes = [];

  for (const crate of CORE_CRATES) {
    const total = sumLines(files.filter((f) => f.filename.includes(crate.marker)));
    if (total.count === 0 && hasSource(crate)) {
      failures.push(
        `${crate.name}: has source code but no measurable lines in the coverage report (minimum ${thresholds.core}%)`,
      );
      continue;
    }
    if (total.count === 0) {
      notes.push(
        `${crate.name}: no measurable lines yet (empty core crate): limit of ${thresholds.core}% applies as soon as code exists`,
      );
      continue;
    }
    const value = pct(total.covered, total.count);
    if (value < thresholds.core) {
      failures.push(`${crate.name}: ${value.toFixed(2)}% of lines (minimum ${thresholds.core}%)`);
    }
  }

  const overall = sumLines(files);
  if (overall.count === 0) {
    notes.push('workspace: no measurable lines yet');
  } else {
    const value = pct(overall.covered, overall.count);
    if (value < thresholds.overall) {
      failures.push(`overall workspace: ${value.toFixed(2)}% of lines (minimum ${thresholds.overall}%)`);
    }
  }

  return { ok: failures.length === 0, failures, notes };
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const path = process.argv[2];
  if (!path) {
    console.error('usage: node scripts/check-rust-coverage.mjs <report.json>');
    process.exit(2);
  }
  const result = evaluateRustCoverage(JSON.parse(readFileSync(path, 'utf8')));
  for (const note of result.notes) console.log(`NOTE ${note}`);
  for (const failure of result.failures) console.error(`FAIL ${failure}`);
  console.log(result.ok ? 'Rust coverage gate: OK' : 'Rust coverage gate: FAILED');
  process.exit(result.ok ? 0 : 1);
}
