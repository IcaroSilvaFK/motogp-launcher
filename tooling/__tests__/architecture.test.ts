// @vitest-environment node
// Issue #1, scenario 3 (frontend side): boundaries via eslint-plugin-boundaries.
import { ESLint } from 'eslint';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const fixtures = join(here, '..', '..', 'tests', 'fixtures', 'boundaries');

async function lint(fixture: string) {
  const cwd = join(fixtures, fixture);
  const eslint = new ESLint({ cwd, overrideConfigFile: join(cwd, 'eslint.config.mjs') });
  const results = await eslint.lintFiles(['src']);
  return results.flatMap((r) =>
    r.messages.map((m) => ({ file: r.filePath.replace(/\\/g, '/'), rule: m.ruleId, message: m.message })),
  );
}

describe('frontend architecture (eslint-plugin-boundaries)', () => {
  it('rejects_ui_importing_infrastructure', async () => {
    const messages = await lint('violations');
    const uiToInfra = messages.filter(
      (m) => m.file.endsWith('src/ui/ImportsInfrastructure.tsx') && m.rule === 'boundaries/dependencies',
    );
    expect(uiToInfra.length).toBeGreaterThan(0);
  });

  it('rejects_domain_importing_outer_layers', async () => {
    const messages = await lint('violations');
    expect(
      messages.some((m) => m.file.endsWith('src/domain/ImportsApplication.ts') && m.rule === 'boundaries/dependencies'),
    ).toBe(true);
  });

  it('rejects_application_importing_infrastructure', async () => {
    const messages = await lint('violations');
    expect(
      messages.some(
        (m) => m.file.endsWith('src/application/ImportsInfrastructure.ts') && m.rule === 'boundaries/dependencies',
      ),
    ).toBe(true);
  });

  it('accepts_composition_root_importing_infrastructure', async () => {
    expect(await lint('valid')).toEqual([]);
  });

  it('rejects_infrastructure_import_from_a_non_root_file_named_main', async () => {
    const messages = await lint('violations');
    expect(messages.some((m) => m.file.endsWith('src/ui/main.tsx') && m.rule === 'boundaries/dependencies')).toBe(true);
  });
});
