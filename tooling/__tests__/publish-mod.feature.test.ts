// Binds tests/features/frontend/publish-mod.feature. It lives under tooling/ because it runs the
// Node script scripts/catalog-entry.mjs (with injected fakes: no network, no real files).
import { describeFeature } from '@amiceli/vitest-cucumber';
import { expect } from 'vitest';
import { run } from '../../scripts/catalog-entry.mjs';
import { validateEntry } from '../../scripts/catalog-schema-check.mjs';
import { loadFrontendFeature } from '../../src/ui/__tests__/features';
import { MOD_BYTES, MOD_SHA256, MOD_URL, REQUIRED_FLAGS, createFakeDeps } from './catalogEntryFixture';

const feature = await loadFrontendFeature('publish-mod');

describeFeature(feature, ({ Scenario }) => {
  Scenario('A creator generates a catalog entry', ({ Given, When, Then, And }) => {
    let fake: ReturnType<typeof createFakeDeps>;
    let argv: string[];
    let code: number;

    Given('a local mod file and its direct HTTPS URL', () => {
      fake = createFakeDeps();
      argv = REQUIRED_FLAGS;
    });

    When('the creator runs the catalog entry script', async () => {
      code = await run(argv, fake.deps);
    });

    Then('it prints an entry with the file size and its lowercase SHA-256', () => {
      expect(code).toBe(0);
      const entry = JSON.parse(fake.out.join(''));
      expect(entry.download).toEqual({ url: MOD_URL, size: MOD_BYTES.length, sha256: MOD_SHA256 });
      expect(entry.download.sha256).toMatch(/^[0-9a-f]{64}$/);
    });

    And('the entry passes the catalog schema check', () => {
      expect(validateEntry(JSON.parse(fake.out.join('')))).toEqual([]);
    });
  });

  Scenario('The hosted file does not match', ({ Given, When, Then }) => {
    let fake: ReturnType<typeof createFakeDeps>;
    let code: number;

    Given('the URL serves different bytes from the local file', () => {
      // Same length, different content: only the hash differs.
      fake = createFakeDeps({ fetchBody: Buffer.from('FAKE MOD ARCHIVE BYTES') });
    });

    When('the script verifies the URL', async () => {
      code = await run([...REQUIRED_FLAGS, '--verify'], fake.deps);
    });

    Then('it fails and reports the size or hash mismatch', () => {
      expect(code).toBe(1);
      expect(fake.out).toEqual([]);
      expect(fake.err.join('')).toMatch(/sha256 mismatch/i);
    });
  });

  Scenario('A non-HTTPS URL is refused', ({ Given, When, Then }) => {
    let fake: ReturnType<typeof createFakeDeps>;
    let argv: string[];
    let code: number;

    Given('the URL uses "http"', () => {
      fake = createFakeDeps();
      argv = REQUIRED_FLAGS.map((a) => (a === MOD_URL ? 'http://example.com/livery.zip' : a));
    });

    When('the creator runs the script', async () => {
      code = await run(argv, fake.deps);
    });

    Then('it fails without printing an entry', () => {
      expect(code).toBe(1);
      expect(fake.out).toEqual([]);
      expect(fake.err.join('')).toMatch(/https/i);
      expect(fake.opened).toEqual([]);
    });
  });
});
