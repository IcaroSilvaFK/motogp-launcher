// Frontend architecture rules (Clean Architecture), shared by the real
// `eslint.config.js` and by the lint fixtures in tests/fixtures/boundaries.
//
//   ui -> application -> domain      infrastructure -> application, domain
//   domain imports nothing from the other layers
//   application never imports infrastructure/ui
//   ui never imports infrastructure
//   src/main.tsx (composition root) is the ONLY file allowed to wire everything.
import boundaries from 'eslint-plugin-boundaries';
import tseslint from 'typescript-eslint';

const layers = ['domain', 'application', 'infrastructure', 'ui'];

export const boundariesConfig = [
  {
    files: ['src/**/*.{ts,tsx}'],
    languageOptions: { parser: tseslint.parser },
    plugins: { boundaries },
    settings: {
      'boundaries/elements': [...layers.map((type) => ({ type, pattern: `src/${type}` }))],
      // The composition root is a single, explicit file (not a layer).
      'boundaries/files': [
        { pattern: 'src/main.tsx', category: 'composition-root' },
        { pattern: '**/*.test.*', category: 'test' },
        { pattern: 'src/test-setup.ts', category: 'test' },
      ],
      'import/resolver': {
        node: { extensions: ['.js', '.jsx', '.ts', '.tsx'] },
      },
    },
    rules: {
      'boundaries/dependencies': [
        'error',
        {
          default: 'disallow',
          checkAllOrigins: true,
          checkUnknownLocals: true,
          policies: [
            { from: { element: { type: 'domain' } }, allow: { to: { element: { type: 'domain' } } } },
            {
              from: { element: { type: 'application' } },
              allow: { to: { element: { types: ['application', 'domain'] } } },
            },
            {
              from: { element: { type: 'infrastructure' } },
              allow: { to: { element: { types: ['infrastructure', 'application', 'domain'] } } },
            },
            {
              from: { element: { type: 'ui' } },
              allow: { to: { element: { types: ['ui', 'application', 'domain'] } } },
            },
            {
              from: { file: { categories: 'composition-root' } },
              allow: { to: [{ element: { types: layers } }, { module: { origin: 'external' } }] },
            },
            {
              from: { element: { types: ['application', 'infrastructure', 'ui'] } },
              allow: { to: { module: { origin: 'external' } } },
            },
            {
              from: { file: { categories: 'test' } },
              allow: { to: { module: { origin: 'external', source: ['vitest', '@testing-library/**'] } } },
            },
            {
              from: { element: { types: ['domain', 'application', 'ui'] } },
              disallow: { to: { module: { origin: 'external', source: '@tauri-apps/**' } } },
            },
          ],
        },
      ],
    },
  },
];
