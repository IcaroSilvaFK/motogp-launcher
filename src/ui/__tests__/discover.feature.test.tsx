// Binds tests/features/frontend/discover.feature: the real App and the real catalog controller over
// a scripted catalog gateway, with a fake launcher controller for the library.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import { createCatalogController } from '../../application';
import type { CatalogController, CatalogGateway, DiscoverView, DiscoverViewStore } from '../../application';
import type { Catalog, CatalogEntry, CommandError, ModEntry } from '../../domain';
import { App } from '../App';
import { en } from '../i18n/en';
import { ptBR } from '../i18n/pt-BR';
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
    screenshots: [],
    longDescription: null,
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
  source: 'remote',
  skipped: 0,
  ...patch,
});

let gateway: { [K in keyof CatalogGateway]: ReturnType<typeof vi.fn> & CatalogGateway[K] };
let catalog: CatalogController;
let user: ReturnType<typeof userEvent.setup>;
let mods: ModEntry[];
let launcher: ReturnType<typeof createFakeController>;

function setGateway(get: () => Promise<Catalog>) {
  gateway = {
    getCatalog: vi.fn(get),
    refreshCatalog: vi.fn(() => Promise.resolve(catalogOf(sample))),
    openCatalogPage: vi.fn(() => Promise.resolve()),
  } as typeof gateway;
  catalog = createCatalogController({ gateway });
}

/** An in-memory view preference store that survives a "restart" (a new render). */
function memoryViewStore(): DiscoverViewStore & { value: DiscoverView | null } {
  const store = {
    value: null as DiscoverView | null,
    load: () => store.value,
    save: (view: DiscoverView) => {
      store.value = view;
    },
  };
  return store;
}

async function openDiscover({
  discoverViewStore,
  locale = 'en',
}: { discoverViewStore?: DiscoverViewStore; locale?: 'en' | 'pt-BR' } = {}) {
  user = userEvent.setup();
  launcher = createFakeController({ mods });
  render(<App controller={launcher} catalog={catalog} locale={locale} discoverViewStore={discoverViewStore} />);
  await user.click(
    within(
      screen.getByRole('navigation', { name: locale === 'en' ? 'Main navigation' : 'Navegação principal' }),
    ).getByRole('button', { name: locale === 'en' ? 'Discover' : 'Descobrir' }),
  );
}

const results = () => screen.getByRole('list', { name: /results/i });
/** Card titles in display order (each card is one button named by its title). */
const titles = () =>
  within(results())
    .getAllByRole('button')
    .map((card) => document.getElementById(card.getAttribute('aria-labelledby') ?? '')?.textContent);
/** The mod page of the open entry (a full page inside Discover since #115, not a dialog). */
const modPage = () => screen.getByRole('region', { name: 'Rebalanced AI' });
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

  Scenario('Entry page and Open page', ({ Given, When, Then }) => {
    let card: HTMLElement;
    Given('the catalog has 2 "Physics" entries and 1 "Liveries" entry', () => withSample());
    When('I open the entry "Rebalanced AI"', async () => {
      card = screen.getByRole('button', { name: 'Rebalanced AI' });
      await user.click(card);
    });
    Then('its page shows the description, version, author and updated date', () => {
      const detail = within(modPage());
      // The page replaces the list: no dialog, and the Discover title is gone.
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      expect(screen.queryByRole('list', { name: /results/i })).not.toBeInTheDocument();
      expect(detail.getByRole('heading', { level: 2, name: 'Rebalanced AI' })).toBeVisible();
      expect(detail.getByText('Smarter and fairer AI riders.')).toBeVisible();
      expect(detail.getByText('2.3.1')).toBeVisible();
      expect(detail.getByText('by Ana Rider')).toBeVisible();
      expect(detail.getByText('Nov 10, 2026')).toBeVisible();
      const details = within(detail.getByRole('region', { name: 'Details' }));
      expect(details.getByText('Ana Rider')).toBeVisible();
      expect(details.getByText('MotoGP 25')).toBeVisible();
      expect(details.getByText('overtake.gg')).toBeVisible();
      expect(detail.getByRole('button', { name: 'Back to Discover' })).toHaveFocus();
    });
    When('I click "Open page"', async () => {
      await user.click(within(modPage()).getByRole('button', { name: 'Open page' }));
    });
    Then('openCatalogPage is called with the entry id', () => {
      expect(gateway.openCatalogPage).toHaveBeenCalledWith('rebalanced-ai');
    });
    When('I press Escape', async () => {
      await user.keyboard('{Escape}');
    });
    Then('the Discover list is shown again with focus on the entry', () => {
      expect(screen.queryByRole('region', { name: 'Rebalanced AI' })).not.toBeInTheDocument();
      expect(screen.getByRole('list', { name: /results/i })).toBeInTheDocument();
      // The list re-mounts, so the card is a new element with the same name.
      card = screen.getByRole('button', { name: 'Rebalanced AI' });
      expect(card).toHaveFocus();
    });
  });

  Scenario('Install is offered only for entries with a download', ({ Given, When, Then }) => {
    Given('an entry with a download block', async () => {
      const download = { url: 'https://github.com/x/y.zip', size: 10, sha256: 'a'.repeat(64) };
      setGateway(() => Promise.resolve(catalogOf([{ ...rebalanced, download }])));
      await openDiscover();
      await screen.findByRole('list', { name: /results/i });
    });
    When('I open the entry "Rebalanced AI"', async () => {
      await user.click(screen.getByRole('button', { name: 'Rebalanced AI' }));
    });
    Then('an Install button is shown', async () => {
      await user.click(within(modPage()).getByRole('button', { name: 'Install' }));
      expect(launcher.installCatalogEntry).toHaveBeenCalledWith('rebalanced-ai');
    });
  });

  Scenario('No Install button for an entry without a download', ({ Given, When, Then }) => {
    Given('an entry without a download block', async () => {
      setGateway(() => Promise.resolve(catalogOf([{ ...rebalanced, download: null }])));
      await openDiscover();
      await screen.findByRole('list', { name: /results/i });
    });
    When('I open the entry "Rebalanced AI"', async () => {
      await user.click(screen.getByRole('button', { name: 'Rebalanced AI' }));
    });
    Then('no Install button is shown', () => {
      expect(within(modPage()).queryByRole('button', { name: /install/i })).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: /install/i })).not.toBeInTheDocument();
    });
  });

  Scenario('Stale banner with the saved date', ({ Given, Then }) => {
    Given('the catalog is stale and was fetched on 2026-11-20', () =>
      withSample({ stale: true, source: 'cache', fetchedAt: Date.UTC(2026, 10, 20, 12) }),
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
      await user.click(within(modPage()).getByRole('button', { name: 'Open page' }));
    });
    Then('an error notice is shown', async () => {
      expect(await within(modPage()).findByRole('alert')).toHaveTextContent(
        'The page of this mod cannot be opened because its address is not trusted.',
      );
      await user.click(within(modPage()).getByRole('button', { name: 'Dismiss' }));
      expect(within(modPage()).queryByRole('alert')).not.toBeInTheDocument();
    });
  });

  Scenario('The view choice survives a restart', ({ Given, When, Then }) => {
    const store = memoryViewStore();
    Given('the user switched Discover to the list view', async () => {
      setGateway(() => Promise.resolve(catalogOf(sample)));
      await openDiscover({ discoverViewStore: store });
      await screen.findByRole('list', { name: /results/i });
      await user.click(screen.getByRole('button', { name: 'List view' }));
      expect(store.value).toBe('list');
    });
    When('the launcher restarts with the same preference store', async () => {
      cleanup();
      setGateway(() => Promise.resolve(catalogOf(sample)));
      await openDiscover({ discoverViewStore: store });
      await screen.findByRole('list', { name: /results/i });
    });
    Then('Discover opens in the list view', () => {
      expect(results()).toHaveAttribute('data-view', 'list');
      expect(screen.getByRole('button', { name: 'List view' })).toHaveAttribute('aria-pressed', 'true');
      expect(screen.getByRole('button', { name: 'Grid view' })).toHaveAttribute('aria-pressed', 'false');
    });
  });

  Scenario('A broken view preference store falls back to the grid', ({ Given, When, Then }) => {
    const broken: DiscoverViewStore = {
      load: vi.fn(() => {
        throw new Error('storage unavailable');
      }),
      save: vi.fn(() => {
        throw new Error('storage unavailable');
      }),
    };
    Given('the view preference store fails to read and write', () => {
      setGateway(() => Promise.resolve(catalogOf(sample)));
    });
    When('the user opens Discover and switches to the list view', async () => {
      await openDiscover({ discoverViewStore: broken });
      await screen.findByRole('list', { name: /results/i });
      expect(results()).toHaveAttribute('data-view', 'grid');
      await user.click(screen.getByRole('button', { name: 'List view' }));
    });
    Then('Discover shows the list view without an error', () => {
      expect(broken.load).toHaveBeenCalled();
      expect(broken.save).toHaveBeenCalledWith('list');
      expect(results()).toHaveAttribute('data-view', 'list');
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });
  });

  Scenario('Discover comes right after Library in the sidebar', ({ Given, Then }) => {
    Given('a catalog is available', async () => {
      setGateway(() => Promise.resolve(catalogOf(sample)));
      render(<App controller={createFakeController({ mods })} catalog={catalog} locale="en" />);
    });
    Then('the sidebar lists Library, Discover, Profiles, Settings in that order', () => {
      const nav = within(screen.getByRole('navigation', { name: 'Main navigation' }));
      expect(nav.getAllByRole('button').map((item) => item.textContent)).toEqual([
        'Library',
        'Discover',
        'Profiles',
        'Settings',
      ]);
    });
  });

  Scenario('Say that the catalog shipped with this version is shown', ({ Given, When, Then, And }) => {
    Given('the backend returns a catalog with source "bundled"', () => {
      setGateway(() => Promise.resolve(catalogOf(sample, { source: 'bundled', stale: true, fetchedAt: 0 })));
    });
    When('the user opens Discover', async () => {
      await openDiscover();
      await screen.findByRole('list', { name: /results/i });
    });
    Then('a notice says the catalog shipped with this version is shown', () => {
      const notice = screen.getByText(/Showing the catalog shipped with this version/).closest('[role="status"]');
      expect(notice).toHaveClass('notice-info');
    });
    And('no "saved on" stale notice is shown', () => {
      expect(screen.queryByText(/saved on/)).not.toBeInTheDocument();
    });
  });

  Scenario('The error state shows only the translated message', ({ Given, And, When, Then }) => {
    let locale: 'en' | 'pt-BR' = 'en';
    let warn: ReturnType<typeof vi.spyOn>;
    Given('the language is Portuguese', () => {
      locale = 'pt-BR';
    });
    And('loading the catalog fails with code "catalog_unavailable" and an English message', () => {
      warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
      const error: CommandError = {
        code: 'catalog_unavailable',
        message: 'The mod catalog could not be loaded (HTTP 404).',
      };
      setGateway(() => Promise.reject(error));
    });
    When('the user opens Discover', async () => {
      await openDiscover({ locale });
    });
    Then('the error state shows the Portuguese text for "catalog_unavailable"', async () => {
      expect(await screen.findByRole('alert')).toHaveTextContent(ptBR.errors.catalog_unavailable);
    });
    And('the English message is not on the screen', () => {
      expect(screen.queryByText(/HTTP 404/)).not.toBeInTheDocument();
    });
    And('the English message is logged as a warning', () => {
      expect(warn).toHaveBeenCalledWith(expect.stringContaining('HTTP 404'));
      warn.mockRestore();
    });
  });

  Scenario('A refresh failure keeps the catalog on screen', ({ Given, When, Then, And }) => {
    Given('a catalog is shown', async () => {
      await withSample();
    });
    When('a refresh fails with code "catalog_unavailable"', async () => {
      vi.spyOn(console, 'warn').mockImplementation(() => undefined);
      gateway.refreshCatalog.mockRejectedValueOnce({ code: 'catalog_unavailable', message: 'HTTP 404' });
      await user.click(screen.getByRole('button', { name: 'Refresh catalog' }));
    });
    Then('the entries stay visible with an inline error', async () => {
      expect(await screen.findByRole('alert')).toHaveTextContent(en.errors.catalog_unavailable);
      expect(screen.queryByText(/HTTP 404/)).not.toBeInTheDocument();
      expect(titles()).toContain('Rebalanced AI');
    });
    And('the full-screen error state is not shown', () => {
      expect(screen.queryByRole('button', { name: 'Retry' })).not.toBeInTheDocument();
      vi.restoreAllMocks();
    });
  });

  Scenario('"Not published" is translated', ({ Given, When, Then }) => {
    Given('loading the catalog fails with code "catalog_not_published"', () => {
      vi.spyOn(console, 'warn').mockImplementation(() => undefined);
      setGateway(() => Promise.reject({ code: 'catalog_not_published', message: 'raw backend text' }));
    });
    When('the user opens Discover', async () => {
      await openDiscover();
    });
    Then('the error state shows the text for "catalog_not_published" in the current language', async () => {
      const alert = await screen.findByRole('alert');
      expect(alert).toHaveTextContent(en.errors.catalog_not_published);
      expect(alert).not.toHaveTextContent('raw backend text');
      vi.restoreAllMocks();
    });
  });
});
