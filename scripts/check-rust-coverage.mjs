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

const BACKSLASH = String.fromCharCode(92);

/**
 * Canonical slash form of a path. Windows paths (drive letter, backslashes or a namespace
 * prefix such as `\?\`, `\??\`, `\?\UNC\`) lose the prefix and are lowercased, since the
 * Windows file system is case-insensitive. POSIX paths stay untouched (case-sensitive).
 */
function canonical(path) {
  const slashed = path.split(BACKSLASH).join('/');
  const unc = /^\/\/\?\/UNC\//i.test(slashed);
  const stripped = slashed.replace(/^\/\/\?\/UNC\//i, '//').replace(/^(\/\/\?\/|\/\?\?\/)/, '');
  const windows = path.includes(BACKSLASH) || /^[a-zA-Z]:\//.test(stripped) || unc;
  return { path: windows ? stripped.toLowerCase() : stripped, windows };
}

/** Path of `file` relative to `root`; canonicalized, unchanged (canonical) when it is not under root. */
function relativeTo(root, file) {
  const r = canonical(root);
  const f = canonical(file);
  const windows = r.windows || f.windows;
  const base = (windows ? r.path.toLowerCase() : r.path).replace(/\/+$/, '');
  const target = windows ? f.path.toLowerCase() : f.path;
  return target.startsWith(`${base}/`) ? target.slice(base.length + 1) : target;
}

/** Removes comments (line, block, nested, doc) from Rust source, keeping string literals intact. */
function stripComments(source) {
  let out = '';
  for (let i = 0; i < source.length;) {
    if (source.startsWith('//', i)) {
      while (i < source.length && source[i] !== '\n') i++;
    } else if (source.startsWith('/*', i)) {
      let depth = 0;
      do {
        if (source.startsWith('/*', i)) {
          depth++;
          i += 2;
        } else if (source.startsWith('*/', i)) {
          depth--;
          i += 2;
        } else i++;
      } while (depth > 0 && i < source.length);
      out += ' ';
    } else if (source[i] === '"') {
      let j = i + 1;
      while (j < source.length && source[j] !== '"') j += source[j] === BACKSLASH ? 2 : 1;
      out += source.slice(i, j + 1);
      i = j + 1;
    } else out += source[i++];
  }
  return out;
}

/** Removes `#[cfg(test)]` items (`mod x;`, `mod x { ... }`, fns, ...) from comment-free source. */
function stripCfgTest(source) {
  let out = source;
  for (;;) {
    const match = /#\[cfg\(test\)\]/.exec(out);
    if (!match) return out;
    let i = match.index + match[0].length;
    while (i < out.length && out[i] !== '{' && out[i] !== ';') i++;
    if (out[i] === '{') {
      let depth = 0;
      do {
        if (out[i] === '"') {
          i++;
          while (i < out.length && out[i] !== '"') i += out[i] === BACKSLASH ? 2 : 1;
        } else if (out[i] === '{') depth++;
        else if (out[i] === '}') depth--;
        i++;
      } while (depth > 0 && i < out.length);
    } else i++;
    out = out.slice(0, match.index) + out.slice(i);
  }
}

/** True when a file has production code: not only comments, cfg(test) items or `mod x;` declarations. */
function hasProductionCode(source) {
  const code = stripCfgTest(stripComments(source)).replace(/^\s*(pub(\([^)]*\))?\s+)?mod\s+\w+\s*;/gm, '');
  return code.trim() !== '';
}

/** True when the crate has non-test Rust code. Child modules are found by walking the folder. */
export function crateHasSource(root, crate) {
  const walk = (dir) =>
    readdirSync(dir, { withFileTypes: true }).some((entry) => {
      const full = join(dir, entry.name);
      if (entry.isDirectory()) return entry.name !== 'tests' && walk(full);
      if (!entry.name.endsWith('.rs') || /^tests?\.rs$/.test(entry.name)) return false;
      return hasProductionCode(readFileSync(full, 'utf8'));
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
