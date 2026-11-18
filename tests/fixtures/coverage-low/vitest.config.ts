// Fixture: same thresholds as the real config, applied to a core layer that is below 90%.
import { defineConfig } from 'vitest/config';
import thresholds from '../../../tooling/coverage-thresholds.json';

export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
    coverage: {
      provider: 'v8',
      include: ['src/**/*.ts'],
      exclude: ['src/**/*.test.ts'],
      thresholds: { lines: thresholds.overall, 'src/domain/**': { lines: thresholds.core } },
    },
  },
});
