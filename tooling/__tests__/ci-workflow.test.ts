// @vitest-environment node
// Issue #1, scenario 4 (CI): the workflow runs the gates and cannot be skipped.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const workflow = readFileSync(join(root, '.github', 'workflows', 'validacao.yml'), 'utf8');

describe('validation workflow', () => {
  it('keeps the required checks and the dev/qas/main triggers', () => {
    expect(workflow).toContain('name: Rust core');
    expect(workflow).toContain('name: Frontend');
    expect(workflow.match(/branches: \[dev, qas, main\]/g)).toHaveLength(2);
  });

  it('runs the frontend gates', () => {
    for (const cmd of ['pnpm prettier --check .', 'pnpm lint', 'pnpm tsc --noEmit', 'pnpm test --coverage']) {
      expect(workflow).toContain(cmd);
    }
  });

  it('does not use continue-on-error', () => {
    expect(workflow).not.toContain('continue-on-error');
  });
});
