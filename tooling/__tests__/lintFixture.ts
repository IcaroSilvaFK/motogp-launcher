import { ESLint } from 'eslint';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const fixtures = join(here, '..', '..', 'tests', 'fixtures', 'boundaries');

export interface LintMessage {
  file: string;
  rule: string | null;
  message: string;
}

// ESLint with the boundaries plugin is slow to start, so each fixture is linted once and shared.
const cache = new Map<string, Promise<LintMessage[]>>();

async function runLint(fixture: string): Promise<LintMessage[]> {
  const cwd = join(fixtures, fixture);
  const eslint = new ESLint({ cwd, overrideConfigFile: join(cwd, 'eslint.config.mjs') });
  const results = await eslint.lintFiles(['src']);
  return results.flatMap((r) =>
    r.messages.map((m) => ({ file: r.filePath.replace(/\\/g, '/'), rule: m.ruleId, message: m.message })),
  );
}

/** Lints a tests/fixtures/boundaries/<fixture> project with the boundaries config (cached). */
export function lint(fixture: string): Promise<LintMessage[]> {
  let pending = cache.get(fixture);
  if (!pending) {
    pending = runLint(fixture);
    cache.set(fixture, pending);
  }
  return pending;
}
