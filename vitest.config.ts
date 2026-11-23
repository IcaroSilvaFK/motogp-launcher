import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';
import thresholds from './tooling/coverage-thresholds.json';

/*
 * Coverage (gate 7): >= 90% lines in src/domain/** and src/application/**,
 * >= 80% overall.
 *
 * Initial condition (issue #1): src/domain and src/application have no
 * executable lines yet (only barrel files with `export {}`), so the 90% rule
 * has nothing to measure. No fictitious business rule was added and the limits
 * were NOT relaxed: as soon as real code lands in those layers, vitest enforces
 * the thresholds on it with no bypass.
 *
 * Narrow exclusions (no business logic): test files, type declarations and
 * `src/main.tsx` (composition root: only mounts React on the real DOM).
 */
export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: false,
    setupFiles: ['./src/test-setup.ts'],
    include: ['src/**/*.test.{ts,tsx}', 'tooling/**/*.test.{ts,tsx}'],
    exclude: ['.worktrees/**', 'node_modules/**', 'tests/fixtures/**', 'target/**', 'dist/**'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'json-summary'],
      include: ['src/**/*.{ts,tsx}'],
      exclude: [
        '.worktrees/**',
        'src/**/*.test.{ts,tsx}',
        'src/**/__tests__/**',
        'src/**/*.d.ts',
        'src/test-setup.ts',
        'src/main.tsx',
      ],
      thresholds: {
        lines: thresholds.overall,
        'src/domain/**': { lines: thresholds.core },
        'src/application/**': { lines: thresholds.core },
      },
    },
  },
});
