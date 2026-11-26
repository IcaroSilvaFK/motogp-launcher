// Binds tests/features/frontend/ads.feature (#104): the real App over a fake launcher controller,
// the real catalog controller over a scripted gateway, and a fake AdProvider. The Tauri adapter's
// mapping of failures to "no ad" is unit-tested in src/infrastructure/__tests__/tauriAdProvider.test.ts.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import { createCatalogController } from '../../application';
import type {
  AdPreferenceStore,
  AdProvider,
  CatalogController,
  LauncherState,
  NavPreferenceStore,
  SessionPhase,
} from '../../application';
import type { AdViewModel, Catalog, CatalogEntry } from '../../domain';
import { App } from '../App';
import { createFakeController } from './fakeController';
import type { FakeController } from './fakeController';
import { loadFrontendFeature } from './features';

const creative: AdViewModel = {
  id: 'creative-42',
  title: 'Track day tyres',
  body: 'Grip for every lap.',
  imageDataUri: 'data:image/png;base64,AAAA',
  sponsorLabel: 'Acme Racing',
};

function entry(id: string, title: string): CatalogEntry {
  return {
    id,
    title,
    author: 'Ana Rider',
    game: 'MotoGP 25',
    category: 'Physics',
    description: 'A mod.',
    version: '1.0.0',
    updatedAt: '2026-11-01',
    pageUrl: `https://overtake.gg/downloads/${id}/`,
    thumbnailUrl: null,
    download: null,
  };
}

const catalogData: Catalog = {
  entries: [entry('rebalanced-ai', 'Rebalanced AI'), entry('tyre-model', 'Tyre Model')],
  fetchedAt: Date.UTC(2026, 10, 24, 12),
  stale: false,
  source: 'remote',
  skipped: 0,
};

/** A configured game and an active profile, so Play is enabled. */
const readyToPlay: Partial<LauncherState> = {
  game: { name: 'MotoGP 25', root: 'C:/Games/MotoGP25', mods_dir: 'C:/Games/MotoGP25/mods', app_id: 2861550 },
  config: { game_dir: 'C:/Games/MotoGP25', active_profile: 'vanilla' },
};

type FakeProvider = { [K in keyof AdProvider]: ReturnType<typeof vi.fn> & AdProvider[K] };

function fakeProvider(get: AdProvider['get'] = () => Promise.resolve(creative)): FakeProvider {
  return {
    get: vi.fn(get),
    click: vi.fn(() => Promise.resolve()),
    reportShown: vi.fn(() => Promise.resolve()),
  } as FakeProvider;
}

interface World {
  controller: FakeController;
  provider: FakeProvider;
  user: ReturnType<typeof userEvent.setup>;
}

let world: World;
let provider: FakeProvider;

function navStore(collapsed: boolean): NavPreferenceStore {
  return { load: () => collapsed, save: () => undefined };
}

/** An in-memory sponsored-content preference (#105); ads show only while it allows them. */
type MemoryPreferenceStore = AdPreferenceStore & { value: 'on' | 'off' | null };

function memoryPreferenceStore(value: 'on' | 'off' | null): MemoryPreferenceStore {
  const store: MemoryPreferenceStore = {
    value,
    load: () => store.value,
    save: (next) => {
      store.value = next;
    },
  };
  return store;
}

let preferenceStore: MemoryPreferenceStore;

function launch({
  state = readyToPlay,
  preference = 'on',
  catalog = createCatalogController({
    gateway: {
      getCatalog: () => Promise.resolve(catalogData),
      refreshCatalog: () => Promise.resolve(catalogData),
      openCatalogPage: () => Promise.resolve(),
    },
  }),
}: { state?: Partial<LauncherState>; preference?: 'on' | 'off'; catalog?: CatalogController } = {}) {
  const controller = createFakeController(state);
  const user = userEvent.setup();
  preferenceStore = memoryPreferenceStore(preference);
  render(
    <App
      controller={controller}
      locale="en"
      navStore={navStore(false)}
      catalog={catalog}
      adPreferenceStore={preferenceStore}
      adProvider={provider}
    />,
  );
  world = { controller, provider, user };
  return world;
}

const nav = () => within(screen.getByRole('navigation', { name: 'Main navigation' }));
const sidebar = () => screen.getByRole('complementary');
const statusBar = () => screen.getByRole('contentinfo');
const adRegions = () => screen.queryAllByRole('region', { name: 'Sponsored' });

async function open(name: 'Discover' | 'Settings' | 'Library') {
  await world.user.click(nav().getByRole('button', { name }));
}

async function openDiscover() {
  await open('Discover');
  await screen.findByRole('list', { name: /results/i });
}

/** The Discover ad: the sponsored region inside the Discover page. */
async function discoverAd() {
  const page = screen.getByRole('region', { name: 'Discover' });
  return within(page).findByRole('region', { name: 'Sponsored' });
}

const SESSION: Record<string, SessionPhase> = {
  starting: { kind: 'starting' },
  running: { kind: 'running', since: null },
  stopping: { kind: 'stopping', since: null },
  'cleaning up': { kind: 'cleaning-up' },
};

const feature = await loadFrontendFeature('ads');

describeFeature(feature, ({ Scenario, ScenarioOutline, BeforeEachScenario, AfterEachScenario }) => {
  BeforeEachScenario(() => {
    provider = fakeProvider();
  });
  AfterEachScenario(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  Scenario('Discover shows a labeled sponsored card', ({ Given, When, Then }) => {
    Given('an ad is available', () => {
      launch();
    });

    When('the user opens Discover', async () => {
      await openDiscover();
    });

    Then('a card labeled "Sponsored" is shown outside the catalog results and counts', async () => {
      const card = await discoverAd();
      expect(within(card).getByText('Sponsored')).toBeVisible();
      expect(within(card).getByText('Track day tyres')).toBeInTheDocument();
      expect(provider.get).toHaveBeenCalledWith('discover');
      // Outside the results list, the category tiles and the toolbar.
      expect(within(screen.getByRole('list', { name: /results/i })).queryByText('Track day tyres')).toBeNull();
      expect(card.closest('.catalog-results, .category-tiles, .catalog-toolbar')).toBeNull();
      // Never counted: the counts stay at the two catalog entries.
      expect(screen.getByText('2 results')).toBeInTheDocument();
      expect(screen.getByText(/^2 mods in the catalog/)).toBeInTheDocument();
      const all = within(screen.getByRole('group', { name: 'Categories' })).getByRole('button', { name: /All/ });
      expect(within(all).getByText('2')).toBeInTheDocument();
      // Never matched by search.
      await world.user.type(screen.getByRole('searchbox'), 'Track day');
      expect(screen.getByText('0 results')).toBeInTheDocument();
    });
  });

  Scenario('The sidebar card hides when collapsed', ({ Given, When, Then }) => {
    Given('an ad is available', async () => {
      launch();
      expect(await within(sidebar()).findByRole('region', { name: 'Sponsored' })).toBeInTheDocument();
      expect(provider.get).toHaveBeenCalledWith('sidebar');
    });

    When('the sidebar is collapsed', async () => {
      await world.user.click(within(sidebar()).getByRole('button', { name: 'Collapse sidebar' }));
      expect(sidebar()).toHaveAttribute('data-collapsed', 'true');
    });

    Then('the sidebar ad is not shown', () => {
      expect(within(sidebar()).queryByRole('region', { name: 'Sponsored' })).toBeNull();
      expect(sidebar().querySelector('.ad-slot')).toBeNull();
    });
  });

  Scenario('Settings About shows a quiet sponsored row', ({ Given, When, Then }) => {
    Given('an ad is available', () => {
      launch();
    });

    When('the user opens Settings > About', async () => {
      await open('Settings');
    });

    Then('a row labeled "Sponsored" is shown in the About card', async () => {
      const about = screen.getByRole('region', { name: 'About' });
      const row = await within(about).findByRole('region', { name: 'Sponsored' });
      expect(row).toHaveClass('setting-row');
      expect(row.closest('.card.rows')).not.toBeNull();
      const action = within(row).getByRole('button', { name: 'Learn more' });
      expect(action).not.toHaveClass('btn-primary');
      expect(provider.get).toHaveBeenCalledWith('about');
    });
  });

  ScenarioOutline('No ads while a game session is active', ({ Given, And, Then }, variables) => {
    Given('an ad is available', () => {
      // The ad is available; the session state is applied before the first render.
    });

    And('the session is "<state>"', () => {
      launch({ state: { ...readyToPlay, session: SESSION[variables.state as string] } });
    });

    Then('no ad is rendered in any screen', async () => {
      await act(() => Promise.resolve());
      expect(adRegions()).toHaveLength(0);
      await openDiscover();
      expect(adRegions()).toHaveLength(0);
      await open('Settings');
      await act(() => Promise.resolve());
      expect(adRegions()).toHaveLength(0);
      // Nothing is fetched during a session either.
      expect(provider.get).not.toHaveBeenCalled();
      // The session ends: the ads come back.
      act(() => world.controller.setState({ session: { kind: 'ended', cleaned: 0 } }));
      expect(await screen.findAllByRole('region', { name: 'Sponsored' })).not.toHaveLength(0);
    });
  });

  Scenario('Play is never blocked', ({ Given, Then, And }) => {
    let resolveAd: (ad: AdViewModel | null) => void = () => undefined;
    let play: HTMLElement;

    Given('an ad is loading', () => {
      provider = fakeProvider(() => new Promise((resolve) => (resolveAd = resolve)));
      launch();
    });

    Then('the Play button is enabled and does not move', async () => {
      play = within(statusBar()).getByRole('button', { name: 'Play' });
      expect(play).toBeEnabled();
      const before = statusBar().innerHTML;
      await act(async () => resolveAd(creative));
      expect(await within(sidebar()).findByRole('region', { name: 'Sponsored' })).toBeInTheDocument();
      // The same node, enabled, and the status bar is unchanged by the ad.
      expect(within(statusBar()).getByRole('button', { name: 'Play' })).toBe(play);
      expect(play).toBeEnabled();
      expect(statusBar().innerHTML).toBe(before);
      expect(statusBar().querySelector('.ad-slot')).toBeNull();
    });

    And('no ad placeholder or spinner is shown', async () => {
      cleanup();
      provider = fakeProvider(() => new Promise(() => undefined));
      launch();
      await openDiscover();
      await open('Settings');
      expect(document.querySelector('.ad-slot')).toBeNull();
      expect(adRegions()).toHaveLength(0);
      expect(screen.queryByRole('status', { name: /sponsored|ad/i })).toBeNull();
      expect(provider.get).toHaveBeenCalled();
    });
  });

  Scenario('Clicking an ad sends only its id', ({ Given, When, Then }) => {
    Given('an ad is shown in the "discover" slot', async () => {
      launch();
      await openDiscover();
      await discoverAd();
    });

    When('the user activates it', async () => {
      const card = await discoverAd();
      within(card).getByRole('button', { name: 'Learn more' }).focus();
      await world.user.keyboard('{Enter}');
    });

    Then('the provider receives a click for slot "discover" and that creative id only', () => {
      expect(provider.click).toHaveBeenCalledTimes(1);
      expect(provider.click).toHaveBeenCalledWith('discover', 'creative-42');
      expect(vi.mocked(provider.click).mock.calls[0]).toEqual(['discover', 'creative-42']);
    });
  });

  Scenario('A shown ad is reported once', ({ Given, When, Then }) => {
    Given('an ad is available', () => {
      launch();
    });

    When('the Discover card is rendered', async () => {
      await openDiscover();
      await discoverAd();
      // Re-renders (typing in the search) must not report it again.
      await world.user.type(screen.getByRole('searchbox'), 'Rebal');
    });

    Then('"shown" is reported once for slot "discover" and that creative id', async () => {
      await waitFor(() => expect(provider.reportShown).toHaveBeenCalledWith('discover', 'creative-42'));
      const discoverCalls = vi.mocked(provider.reportShown).mock.calls.filter(([slot]) => slot === 'discover');
      expect(discoverCalls).toEqual([['discover', 'creative-42']]);
    });
  });

  Scenario('No ad means no slot', ({ Given, Then }) => {
    const providers: FakeProvider[] = [];

    Given('the provider has no ad, or it fails, or the command does not exist', () => {
      vi.spyOn(console, 'warn').mockImplementation(() => undefined);
      providers.push(
        fakeProvider(() => Promise.resolve(null)),
        fakeProvider(() => Promise.reject(new Error('feed unreachable'))),
        // What a webview without the backend command rejects with.
        fakeProvider(() => Promise.reject('command get_ad not found')),
      );
    });

    Then('no slot is rendered and no error is shown to the user', async () => {
      for (const next of providers) {
        cleanup();
        provider = next;
        launch();
        await waitFor(() => expect(next.get).toHaveBeenCalledWith('sidebar'));
        await openDiscover();
        await open('Settings');
        await act(() => Promise.resolve());
        expect(document.querySelector('.ad-slot')).toBeNull();
        expect(adRegions()).toHaveLength(0);
        expect(screen.queryByRole('alert')).toBeNull();
        expect(screen.queryByText(/feed unreachable|not found/)).toBeNull();
      }
    });
  });

  Scenario('Ads disabled renders nothing', ({ Given, And, When, Then }) => {
    Given('an ad is available', () => {
      // The default provider resolves a creative; the stored preference is applied at launch.
    });

    And('the user turned sponsored content off', () => {
      launch({ preference: 'off' });
    });

    When('the user opens Discover and Settings > About', async () => {
      await act(() => Promise.resolve());
      expect(within(sidebar()).queryByRole('region', { name: 'Sponsored' })).toBeNull();
      await openDiscover();
      expect(adRegions()).toHaveLength(0);
      await open('Settings');
      await act(() => Promise.resolve());
    });

    Then('no slot renders and no ad request is made', () => {
      expect(
        within(screen.getByRole('region', { name: 'About' })).queryByRole('region', { name: 'Sponsored' }),
      ).toBeNull();
      expect(document.querySelector('.ad-slot')).toBeNull();
      expect(provider.get).not.toHaveBeenCalled();
      expect(provider.reportShown).not.toHaveBeenCalled();
    });
  });

  Scenario('A creative without an image', ({ Given, When, Then }) => {
    let card: HTMLElement;

    Given('a house ad has no image', () => {
      provider = fakeProvider(() =>
        Promise.resolve({ ...creative, imageDataUri: null, sponsorLabel: 'MotoGP Launcher' }),
      );
    });

    When('it is shown in the sidebar', async () => {
      launch();
      card = await within(sidebar()).findByRole('region', { name: 'Sponsored' });
    });

    Then('a compact card shows an icon tile, the title and the body', () => {
      expect(card).toHaveClass('ad-slot-sidebar', 'ad-text-first');
      expect(card.querySelector('img')).toBeNull();
      const tile = card.querySelector('.ad-tile');
      expect(tile).not.toBeNull();
      expect(tile).toHaveAttribute('aria-hidden', 'true');
      expect(within(card).getByText('Track day tyres')).toHaveClass('ad-title');
      expect(within(card).getByText('Grip for every lap.')).toHaveClass('ad-text');
      expect(within(card).getAllByRole('button')).toHaveLength(1);
    });
  });

  Scenario('One label only', ({ Given, When, Then }) => {
    Given('an ad from "Acme Racing" is available', () => {
      launch();
    });

    When('the user opens Discover', async () => {
      await openDiscover();
    });

    Then('the card shows "Sponsored" once and the sponsor name as a byline', async () => {
      const card = await discoverAd();
      expect(within(card).getAllByText(/sponsored/i)).toHaveLength(1);
      const sponsor = within(card).getByText('Acme Racing');
      expect(sponsor).toHaveClass('ad-byline');
      // Never an equal label next to "Sponsored".
      expect(sponsor.closest('.ad-label')).toBeNull();
      expect(card.querySelector('.ad-label .dot')).toBeNull();
    });
  });

  Scenario('Turning sponsored content off in Settings removes the slots', ({ Given, When, Then }) => {
    Given('an ad is shown in the sidebar', async () => {
      launch();
      expect(await within(sidebar()).findByRole('region', { name: 'Sponsored' })).toBeInTheDocument();
    });

    When('the user selects "Off" in Settings > Preferences', async () => {
      await open('Settings');
      expect(
        await within(screen.getByRole('region', { name: 'About' })).findByRole('region', { name: 'Sponsored' }),
      ).toBeInTheDocument();
      await world.user.selectOptions(screen.getByLabelText('Sponsored content'), 'off');
    });

    Then('no slot renders in the sidebar, Discover or Settings > About', async () => {
      expect(preferenceStore.value).toBe('off');
      expect(adRegions()).toHaveLength(0);
      provider.get.mockClear();
      await openDiscover();
      await act(() => Promise.resolve());
      expect(adRegions()).toHaveLength(0);
      expect(document.querySelector('.ad-slot')).toBeNull();
      expect(provider.get).not.toHaveBeenCalled();
    });
  });
});
