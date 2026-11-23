// Binds tests/features/frontend/scaffold.feature (it lives under tooling/ because it runs Node tooling, which the src/ui boundaries rightly forbid). The shell render runs against the real App; the
// boundary and coverage scenarios run the real tooling (ESLint with eslint-plugin-boundaries on
// the tests/fixtures/boundaries projects, and the coverage gate evaluator). The ui -> infrastructure
// scenario is bound only here; architecture.test.ts holds additional boundary cases and
// coverage.test.ts deeper coverage-gate unit tests.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen } from '@testing-library/react';
import { expect, vi } from 'vitest';
import { evaluateRustCoverage } from '../../scripts/check-rust-coverage.mjs';
import { lint } from './lintFixture';
import type { LintMessage } from './lintFixture';
import { App } from '../../src/ui/App';
import { createFakeController } from '../../src/ui/__tests__/fakeController';
import { loadFrontendFeature } from '../../src/ui/__tests__/features';

// ESLint with the boundaries plugin is slow to start.
vi.setConfig({ testTimeout: 30_000 });

const feature = await loadFrontendFeature('scaffold');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  Scenario('Render the launcher shell', ({ Given, When, Then }) => {
    Given('the launcher started in the WebView', () => {
      expect(document.body).toBeInTheDocument();
    });

    When('the interface is rendered', () => {
      render(<App controller={createFakeController()} />);
    });

    Then('I see the accessible MotoGP Launcher shell', () => {
      expect(screen.getByRole('banner')).toBeInTheDocument();
      expect(screen.getByRole('heading', { level: 1, name: 'MotoGP Launcher' })).toBeInTheDocument();
      expect(screen.getByRole('main')).toBeInTheDocument();
    });
  });

  Scenario('Reject an import from ui to infrastructure', ({ Given, When, Then, And }) => {
    let violations: LintMessage[] = [];
    let valid: LintMessage[] = [];

    Given('an import from ui to infrastructure in a lint fixture', () => {
      // tests/fixtures/boundaries/violations/src/ui/ImportsInfrastructure.tsx
    });

    When('I run ESLint with eslint-plugin-boundaries', async () => {
      violations = await lint('violations');
      valid = await lint('valid');
    });

    Then('the violation is rejected', () => {
      const uiToInfra = violations.filter(
        (m) => m.file.endsWith('src/ui/ImportsInfrastructure.tsx') && m.rule === 'boundaries/dependencies',
      );
      expect(uiToInfra.length).toBeGreaterThan(0);
    });

    And('the src/main.tsx composition root remains valid', () => {
      expect(valid).toEqual([]);
    });
  });

  Scenario('Insufficient coverage fails the gate', ({ Given, When, Then }) => {
    let report: Parameters<typeof evaluateRustCoverage>[0];
    let result: ReturnType<typeof evaluateRustCoverage>;

    Given('a coverage report below the thresholds', () => {
      const file = (filename: string, covered: number, count: number) => ({
        filename,
        summary: { lines: { count, covered, percent: (covered / count) * 100 } },
      });
      report = {
        data: [
          {
            files: [
              file('crates/domain/src/a.rs', 100, 100),
              file('crates/application/src/b.rs', 85, 100),
              file('crates/infrastructure/src/c.rs', 300, 300),
            ],
          },
        ],
      };
    });

    When('the coverage gate is evaluated', () => {
      result = evaluateRustCoverage(report, { root: '/repo', hasSource: () => false });
    });

    Then('the validation fails', () => {
      expect(result.ok).toBe(false);
      expect(result.failures.join('\n')).toMatch(/motogp-application.*85/);
    });
  });
});
