import js from '@eslint/js';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';
import tseslint from 'typescript-eslint';
import { boundariesConfig } from './tooling/boundaries.config.mjs';

export default tseslint.config(
  {
    ignores: [
      '.worktrees/**',
      'dist/**',
      'coverage/**',
      'target/**',
      'node_modules/**',
      'src-tauri/**',
      // Lint fixtures contain violations ON PURPOSE (see tooling/__tests__).
      'tests/fixtures/**',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: { globals: globals.browser },
    plugins: { 'react-hooks': reactHooks },
    rules: reactHooks.configs.recommended.rules,
  },
  {
    files: ['**/*.{js,mjs}', 'vitest.config.ts', 'vite.config.ts', 'tooling/**/*.ts'],
    languageOptions: { globals: globals.node },
  },
  ...boundariesConfig,
);
