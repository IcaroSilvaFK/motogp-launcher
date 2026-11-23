// @vitest-environment node
// Issue #1, scenario 3 (frontend side): boundaries via eslint-plugin-boundaries.
// The scenario "Reject an import from ui to infrastructure" itself is bound in
// scaffold.feature.test.tsx; this file holds the extra boundary cases it does not cover.
import { describe, expect, it } from 'vitest';
import { lint } from './lintFixture';

// ESLint with the boundaries plugin is slow to start: the first lint of a fixture is slow.
const TEST_TIMEOUT_MS = 30_000;

describe('frontend architecture (eslint-plugin-boundaries)', { timeout: TEST_TIMEOUT_MS }, () => {
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

  it('rejects_infrastructure_import_from_a_non_root_file_named_main', async () => {
    const messages = await lint('violations');
    expect(messages.some((m) => m.file.endsWith('src/ui/main.tsx') && m.rule === 'boundaries/dependencies')).toBe(true);
  });

  it.each([
    'src/domain/ImportsTauri.ts',
    'src/domain/ImportsReact.ts',
    'src/application/ImportsTauri.ts',
    'src/ui/ImportsTauri.tsx',
    'src/ui/ImportsUnclassified.tsx',
  ])('rejects an adapter boundary bypass in %s', async (file) => {
    const messages = await lint('violations');
    expect(messages.some((m) => m.file.endsWith(file) && m.rule === 'boundaries/dependencies')).toBe(true);
  });
});
