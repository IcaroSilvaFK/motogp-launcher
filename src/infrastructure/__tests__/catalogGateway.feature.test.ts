// Binds tests/features/frontend/catalog-gateway.feature: the Tauri gateway over a fake `invoke`
// (the camelCase DTO passes through untouched).
import { describeFeature, loadFeature } from '@amiceli/vitest-cucumber';
import { expect, vi } from 'vitest';
import type { Catalog } from '../../domain';
import { createTauriGateway } from '../tauriGateway';

const feature = await loadFeature(`${import.meta.dirname}/../../../tests/features/frontend/catalog-gateway.feature`);

const catalog: Catalog = {
  entries: [
    {
      id: 'rebalanced-ai',
      title: 'Rebalanced AI',
      author: 'Someone',
      game: 'MotoGP 26',
      category: 'AI',
      description: 'Fairer riders.',
      version: '1.2',
      updatedAt: '2026-11-01',
      pageUrl: 'https://example.com/rebalanced-ai',
      thumbnailUrl: null,
      download: null,
    },
  ],
  fetchedAt: 1_700_000_000_000,
  stale: false,
  source: 'remote',
  skipped: 0,
};

describeFeature(feature, ({ Scenario }) => {
  Scenario('The gateway calls the catalog commands', ({ When, Then }) => {
    const invoke = vi.fn().mockResolvedValue(catalog);
    const results: Catalog[] = [];

    When('getCatalog and refreshCatalog are called', async () => {
      const gateway = createTauriGateway(invoke);
      results.push(await gateway.getCatalog(), await gateway.refreshCatalog());
    });

    Then('"get_catalog" and "refresh_catalog" are invoked', () => {
      expect(invoke.mock.calls).toEqual([['get_catalog'], ['refresh_catalog']]);
      expect(results).toEqual([catalog, catalog]);
    });
  });

  Scenario('The gateway opens a catalog page by id', ({ When, Then }) => {
    const invoke = vi.fn().mockResolvedValue(undefined);

    When('openCatalogPage is called with "rebalanced-ai"', async () => {
      await createTauriGateway(invoke).openCatalogPage('rebalanced-ai');
    });

    Then('"open_catalog_page" is invoked with only the id', () => {
      expect(invoke.mock.calls).toEqual([['open_catalog_page', { id: 'rebalanced-ai' }]]);
    });
  });
});
