// Binds tests/features/frontend/discover.feature: the real App and the real catalog controller over
// a scripted catalog gateway, with a fake launcher controller for the library.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import { createCatalogController } from '../../application';
import type { CatalogController, CatalogGateway } from '../../application';
import type { Catalog, CatalogEntry, CommandError, ModEntry } from '../../domain';
import { App } from '../App';
import { createFakeController } from './fakeController';
import { loadFrontendFeature } from './features';

function entry(patch: Partial<CatalogEntry> & { id: string; title: string }): CatalogEntry {
  return {
    author: 'Ana Rider',
    game: 'MotoGP 25',
    category: 'Physics',
    description: 'A mod.',
    version: '1.0.0',
    updatedAt: '2026-01-01',
    pageUrl: `https://overtake.gg/downloads/${patch.id}/`,
    thumbnailUrl: null,
    download: null,
    ...patch,
  };
}

const rebalanced = entry({
  id: 'rebalanced-ai',
  title: 'Rebalanced AI',
  description: 'Smarter and fairer AI riders.',
  version: '2.3.1',
  updatedAt: '2026-11-10',
});
const tyres = entry({ id: 'tyre-model', title: 'Tyre Model', author: 'Grip Lab', updatedAt: '2026-11-20' });
const livery = entry({
  id: 'yamaha-livery',
  title: 'Classic Yamaha Livery',
  author: 'Paint Shop',
  category: 'Liveries',
  updatedAt: '2026-09-01',
});
const sample = [rebalanced, tyres, livery];

const catalogOf = (entries: CatalogEntry[], patch: Partial<Catalog> = {}): Catalog => ({
  entries,
  fetchedAt: Date.UTC(2026, 10, 24, 12),
  stale: false,
  skipped: 0,
  ...patch,
});

let gateway: { [K in keyof CatalogGateway]: ReturnType<typeof vi.fn> & CatalogGateway[K] };
let catalog: CatalogController;
let user: ReturnType<typeof userEvent.setup>;
let mods: ModEntry[];

function setGateway(get: () => Promise<Catalog>) {
  gateway = {
    getCatalog: vi.fn(get),
    refreshCatalog: vi.fn(() => Promise.resolve(catalogOf(sample))),
    openCatalogPage: vi.fn(() => Promise.resolve()),
  } as typeof gateway;
  catalog = createCatalogController({ gateway });
}

async function openDiscover() {
  user = userEvent.setup();
  render(<App controller={createFakeController({ mods })} catalog={catalog} locale="en" />);
  await user.click(
    within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', { name: 'Discover' }),
  );
}

const results = () => screen.getByRole('list', { name: /results/i });
/** Card titles in display order (each card is one button named by its title). */
const titles = () =>
  within(results())
    .getAllByRole('button')
    .map((card) => document.getElementById(card.getAttribute('aria-labelledby') ?? '')?.textContent);
const dialog = () => screen.getByRole('dialog');
/** The list item of an entry card. */
const cardOf = (title: string) => screen.getByRole('button', { name: title }).closest('li') as HTMLElement;

async function withSample(patch: Partial<Catalog> = {}) {
  setGateway(() => Promise.resolve(catalogOf(sample, patch)));
  await openDiscover();
  await screen.findByRole('list', { name: /results/i });
}

const feature = await loadFrontendFeature('discover');

describeFeature(feature, ({ Scenario, BeforeEachScenario, AfterEachScenario }) => {
  BeforeEachScenario(() => {
    mods = [];
  });
  AfterEachScenario(cleanup);

  Scenario('Reach Discover from the sidebar', ({ When, Then }) => {
    When('I click "Discover" in the sidebar', async () => {
      setGateway(() => Promise.resolve(catalogOf(sample)));
      await openDiscover();
    });
    Then('the Discover screen is shown and the catalog is requested', async () => {
      expect(screen.getByRole('heading', { level: 2, name: 'Discover' })).toBeVisible();
      const nav = within(screen.getByRole('navigation', { name: 'Main navigation' }));
      expect(nav.getAllByRole('button').map((item) => item.textContent)).toEqual([
        'Library',
        'Discover',
        'Profiles',
        'Settings',
      ]);
      expect(nav.getByRole('button', { name: 'Discover' })).toHaveAttribute('aria-current', 'page');
      expect(gateway.getCatalog).toHaveBeenCalledTimes(1);
      expect(await screen.findByText('3 mods in the catalog', { exact: false })).toBeInTheDocument();
    });
  });

  Scenario('Loading skeleton', ({ Given, Then }) => {
    Given('the catalog request is pending', async () => {
      setGateway(() => new Promise<Catalog>(() => undefined));
      await openDiscover();
    });
    Then('a loading skeleton is shown', () => {
      const page = screen.getByRole('region', { name: 'Discover' });
      expect(page).toHaveAttribute('aria-busy', 'true');
      expect(within(page).getByRole('status')).toHaveTextContent('Loading the catalog');
      expect(page.querySelectorAll('.skeleton').length).toBeGreaterThan(0);
    });
  });

  Scenario('Empty catalog explains how mods get listed', ({ Given, Then, And }) => {
    Given('the catalog has no entries', async () => {
      setGateway(() => Promise.resolve(catalogOf([])));
      gateway.refreshCatalog.mockImplementation(() => Promise.resolve(catalogOf([])));
      await openDiscover();
    });
    Then('an empty state explains that mods are added to the curated catalog by pull request', async () => {
      expect(await screen.findByText('The catalog is empty for now')).toBeVisible();
      expect(screen.getByText(/curated catalog maintained in the launcher's GitHub repository/)).toBeVisible();
      expect(screen.getByText(/pull request/)).toHaveTextContent('catalog/catalog.json');
      expect(screen.getByText(/drag and drop/)).toBeVisible();
      expect(screen.queryByRole('link')).not.toBeInTheDocument();
    });
    And('it offers a refresh action', async () => {
      await user.click(screen.getByRole('button', { name: 'Refresh catalog' }));
      expect(gateway.refreshCatalog).toHaveBeenCalledTimes(1);
    });
    And('it offers a way to go to the Library', async () => {
      await user.click(screen.getByRole('button', { name: 'Go to Library' }));
      expect(screen.getByRole('heading', { level: 2, name: 'Library' })).toBeVisible();
    });
  });

  Scenario('Browse categories with counts', ({ Given, Then, When }) => {
    Given('the catalog has 2 "Physics" entries and 1 "Liveries" entry', () => withSample());
    Then('the category tiles show "Physics 2" and "Liveries 1"', () => {
      const tiles = within(screen.getByRole('group', { name: 'Categories' })).getAllByRole('button');
      expect(tiles.map((tile) => tile.textContent)).toEqual(['All 3', 'Liveries 1', 'Physics 2']);
      expect(screen.getByRole('button', { name: 'Physics 2' })).toHaveAttribute('aria-pressed', 'false');
      expect(screen.getByRole('button', { name: 'Liveries 1' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'All 3' })).toHaveAttribute('aria-pressed', 'true');
    });
    When('I pick "Physics"', async () => {
      await user.click(screen.getByRole('button', { name: 'Physics 2' }));
    });
    Then('only the 2 Physics entries are listed', () => {
      expect(screen.getByRole('button', { name: 'Physics 2' })).toHaveAttribute('aria-pressed', 'true');
      expect(titles()).toEqual(['Tyre Model', 'Rebalanced AI']);
    });
  });

  Scenario('Search by title, author or category', ({ Given, When, Then }) => {
    Given('the catalog has 2 "Physics" entries and 1 "Liveries" entry', () => withSample());
    When('I search "rebal"', async () => {
      await user.type(screen.getByRole('searchbox', { name: 'Search the catalog' }), 'rebal');
    });
    Then('only matching entries are listed', async () => {
      expect(titles()).toEqual(['Rebalanced AI']);
      await user.clear(screen.getByRole('searchbox', { name: 'Search the catalog' }));
      await user.type(screen.getByRole('searchbox', { name: 'Search the catalog' }), 'zzz');
      expect(screen.getByText('No mods match your search.')).toBeVisible();
      await user.click(screen.getByRole('button', { name: 'Clear filters' }));
      expect(titles()).toHaveLength(3);
    });
  });

  Scenario('Sort by recently updated or name', ({ Given, Then, When }) => {
    Given('the catalog has 2 "Physics" entries and 1 "Liveries" entry', () => withSample());
    Then('entries are listed by most recently updated first', () => {
      expect(screen.getByRole('combobox', { name: 'Sort by' })).toHaveValue('updated');
      expect(titles()).toEqual(['Tyre Model', 'Rebalanced AI', 'Classic Yamaha Livery']);
    });
    When('I sort by name', async () => {
      await user.selectOptions(screen.getByRole('combobox', { name: 'Sort by' }), 'name');
    });
    Then('entries are listed alphabetically', () => {
      expect(titles()).toEqual(['Classic Yamaha Livery', 'Rebalanced AI', 'Tyre Model']);
    });
  });

  Scenario('Switch grid and list views', ({ Given, When, Then }) => {
    Given('the catalog has 2 "Physics" entries and 1 "Liveries" entry', async () => {
      await withSample();
      expect(results()).toHaveAttribute('data-view', 'grid');
      expect(screen.getByRole('button', { name: 'Grid view' })).toHaveAttribute('aria-pressed', 'true');
    });
    When('I switch to the list view', async () => {
      await user.click(screen.getByRole('button', { name: 'List view' }));
    });
    Then('entries are shown as rows', () => {
      expect(results()).toHaveAttribute('data-view', 'list');
      expect(screen.getByRole('button', { name: 'List view' })).toHaveAttribute('aria-pressed', 'true');
      expect(screen.getByRole('button', { name: 'Grid view' })).toHaveAttribute('aria-pressed', 'false');
      expect(within(results()).getAllByRole('listitem')).toHaveLength(3);
    });
  });

  Scenario('Entry detail and Open page', ({ Given, When, Then }) => {
    let card: HTMLElement;
    Given('the catalog has 2 "Physics" entries and 1 "Liveries" entry', () => withSample());
    When('I open the entry "Rebalanced AI"', async () => {
      card = screen.getByRole('button', { name: 'Rebalanced AI' });
      await user.click(card);
    });
    Then('its description, version, author and updated date are shown', () => {
      const detail = within(dialog());
      expect(dialog()).toHaveAttribute('aria-modal', 'true');
      expect(detail.getByRole('heading', { name: 'Rebalanced AI' })).toBeVisible();
      expect(detail.getByText('Smarter and fairer AI riders.')).toBeVisible();
      expect(detail.getByText('2.3.1')).toBeVisible();
      expect(detail.getByText('Ana Rider')).toBeVisible();
      expect(detail.getByText('Nov 10, 2026')).toBeVisible();
      expect(detail.getByText('MotoGP 25')).toBeVisible();
      expect(detail.getByText('overtake.gg')).toBeVisible();
      expect(detail.getByRole('button', { name: 'Close' })).toHaveFocus();
    });
    When('I click "Open page"', async () => {
      await user.click(within(dialog()).getByRole('button', { name: 'Open page' }));
    });
    Then('openCatalogPage is called with the entry id', () => {
      expect(gateway.openCatalogPage).toHaveBeenCalledWith('rebalanced-ai');
    });
    When('I press Escape', async () => {
      await user.keyboard('{Escape}');
    });
    Then('the entry detail is closed', () => {
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      expect(card).toHaveFocus();
    });
  });

  Scenario('Install stays hidden until downloads exist', ({ Given, When, Then }) => {
    Given('an entry with a download block', async () => {
      const download = { url: 'https://github.com/x/y.zip', size: 10, sha256: 'a'.repeat(64) };
      setGateway(() => Promise.resolve(catalogOf([{ ...rebalanced, download }])));
      await openDiscover();
      await screen.findByRole('list', { name: /results/i });
    });
    When('I open the entry "Rebalanced AI"', async () => {
      await user.click(screen.getByRole('button', { name: 'Rebalanced AI' }));
    });
    Then('no Install button is shown', () => {
      expect(within(dialog()).queryByRole('button', { name: /install/i })).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /install/i })).not.toBeInTheDocument();
    });
  });

  Scenario('Stale banner with the saved date', ({ Given, Then }) => {
    Given('the catalog is stale and was fetched on 2026-11-20', () =>
      withSample({ stale: true, fetchedAt: Date.UTC(2026, 10, 20, 12) }),
    );
    Then('a banner says the catalog saved on that date is shown', () => {
      const banner = screen.getByText(/Showing the catalog saved on/).closest('[role="status"]');
      expect(banner).toHaveTextContent('Showing the catalog saved on Nov 20, 2026');
      expect(banner).toHaveClass('notice-warning');
    });
  });

  Scenario('Error with retry', ({ Given, Then, When }) => {
    Given('the catalog request fails with "catalog_unavailable"', async () => {
      const error: CommandError = { code: 'catalog_unavailable', message: '' };
      setGateway(() => Promise.reject(error));
      await openDiscover();
    });
    Then('an error with a Retry button is shown', async () => {
      expect(await screen.findByRole('alert')).toHaveTextContent('The mod catalog could not be loaded.');
      expect(screen.getByRole('button', { name: 'Retry' })).toBeVisible();
    });
    When('I click Retry', async () => {
      await user.click(screen.getByRole('button', { name: 'Retry' }));
    });
    Then('refreshCatalog is called', async () => {
      expect(gateway.refreshCatalog).toHaveBeenCalledTimes(1);
      expect(await screen.findByRole('list', { name: /results/i })).toBeInTheDocument();
    });
  });

  Scenario('Already in library badge', ({ Given, Then }) => {
    Given('the library has a mod matching the entry "rebalanced-ai"', async () => {
      mods = [{ id: 'local-1', name: 'rebalanced ai', files: ['a.pak'], size_bytes: 1 }];
      await withSample();
    });
    Then('the entry shows "In library"', () => {
      expect(within(cardOf('Rebalanced AI')).getByText('In library')).toBeVisible();
      expect(within(cardOf('Tyre Model')).queryByText('In library')).toBeNull();
    });
  });

  Scenario('Open page failure is shown', ({ Given, When, Then, And }) => {
    Given('opening the page fails with "catalog_page_not_allowed"', async () => {
      await withSample();
      gateway.openCatalogPage.mockImplementation(() =>
        Promise.reject({ code: 'catalog_page_not_allowed', message: '' } satisfies CommandError),
      );
    });
    When('I open the entry "Rebalanced AI"', async () => {
      await user.click(screen.getByRole('button', { name: 'Rebalanced AI' }));
    });
    And('I click "Open page"', async () => {
      await user.click(within(dialog()).getByRole('button', { name: 'Open page' }));
    });
    Then('an error notice is shown', async () => {
      expect(await within(dialog()).findByRole('alert')).toHaveTextContent(
        'The page of this mod cannot be opened because its address is not trusted.',
      );
      await user.click(within(dialog()).getByRole('button', { name: 'Dismiss' }));
      expect(within(dialog()).queryByRole('alert')).not.toBeInTheDocument();
    });
  });
});
