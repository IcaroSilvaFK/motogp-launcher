// Binds tests/features/frontend/download-counts.feature (#120): the real App over the real launcher
// and catalog controllers, a scripted DownloadStats port and the install report wired exactly as
// main.tsx wires it (onCatalogInstalled -> reportCatalogDownload, gated by the usage preference).
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import {
  createCatalogController,
  createLauncherController,
  createUsagePreference,
  reportCatalogDownload,
} from '../../application';
import type { CatalogGateway, DownloadStats, LauncherController, UsagePreferenceStore } from '../../application';
import type { Catalog, CatalogEntry, DailyDownloads, DownloadTotal } from '../../domain';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import { App } from '../App';
import { loadFrontendFeature } from './features';

const ID = 'rebalanced-ai';

function entry(id: string, title: string): CatalogEntry {
  return {
    id,
    title,
    author: 'Ana Rider',
    game: 'MotoGP 25',
    category: 'physics',
    description: 'A mod.',
    version: '1.0.0',
    updatedAt: '2026-11-01',
    pageUrl: `https://overtake.gg/downloads/${id}/`,
    thumbnailUrl: null,
    screenshots: [],
    longDescription: null,
    download: null,
  };
}

const ENTRIES = [entry(ID, 'Rebalanced AI'), entry('tyre-model', 'Tyre Model')];
const TOTALS: DownloadTotal[] = [{ id: ID, total: 1234, last30Days: 56 }];
/** 30 UTC days from 2026-10-28 to 2026-11-26: zero except a peak of 12 on Nov 20 and 3 on Nov 26. */
const DAILY: DailyDownloads[] = Array.from({ length: 30 }, (_, index) => ({
  date: new Date(Date.UTC(2026, 9, 28 + index)).toISOString().slice(0, 10),
  count: index === 23 ? 12 : index === 29 ? 3 : 0,
}));

function memoryStore(initial: 'on' | 'off' | null) {
  const memory: { value: 'on' | 'off' | null } = { value: initial };
  return Object.assign(memory, {
    load: () => memory.value,
    save: (value: 'on' | 'off') => {
      memory.value = value;
    },
  }) satisfies UsagePreferenceStore;
}

function statsPort(online: boolean) {
  return {
    report: vi.fn(() => Promise.resolve()),
    totals: vi.fn(() => Promise.resolve(online ? TOTALS : null)),
    daily: vi.fn(() => Promise.resolve(online ? DAILY : null)),
  } satisfies DownloadStats;
}

const catalogOf = (entries: CatalogEntry[]): Catalog => ({
  entries,
  fetchedAt: Date.UTC(2026, 10, 24, 12),
  stale: false,
  source: 'remote',
  skipped: 0,
});

interface World {
  gateway: FakeGateway;
  controller: LauncherController;
  stats: ReturnType<typeof statsPort>;
  store: ReturnType<typeof memoryStore>;
  user: ReturnType<typeof userEvent.setup>;
}

/** Composes the app as main.tsx does, with a real launcher controller over a fake gateway. */
async function launch({ online = true, stored = null as 'on' | 'off' | null } = {}): Promise<World> {
  const gateway = new FakeGateway();
  const stats = statsPort(online);
  const store = memoryStore(stored);
  const usagePreference = createUsagePreference(store);
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
    onCatalogInstalled: (id) => reportCatalogDownload({ stats, preference: usagePreference }, id),
  });
  const catalogGateway: CatalogGateway = {
    getCatalog: vi.fn(() => Promise.resolve(catalogOf(ENTRIES))),
    refreshCatalog: vi.fn(() => Promise.resolve(catalogOf(ENTRIES))),
    openCatalogPage: vi.fn(() => Promise.resolve()),
  };
  const user = userEvent.setup();
  render(
    <App
      controller={controller}
      catalog={createCatalogController({ gateway: catalogGateway })}
      locale="en"
      downloadStats={stats}
      usagePreference={usagePreference}
    />,
  );
  await screen.findByRole('navigation', { name: 'Main navigation' });
  return { gateway, controller, stats, store, user };
}

const nav = () => within(screen.getByRole('navigation', { name: 'Main navigation' }));

async function openSettings(world: World) {
  await world.user.click(nav().getByRole('button', { name: 'Settings' }));
  return screen.findByRole('switch', { name: 'Share anonymous usage' });
}

async function openDiscover(world: World) {
  await world.user.click(nav().getByRole('button', { name: 'Discover' }));
  return screen.findByRole('list', { name: /results/i });
}

async function openMod(world: World, title: string) {
  await world.user.click(screen.getByRole('button', { name: title }));
  return screen.getByRole('region', { name: title });
}

/** Lets the fire-and-forget calls and their effects settle. */
const settle = () => act(async () => undefined);

const feature = await loadFrontendFeature('download-counts');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  Scenario('Usage sharing is on by default', ({ Given, When, Then, And }) => {
    let world: World;
    let toggle: HTMLElement;
    Given('the user never chose a usage preference', async () => {
      world = await launch({ stored: null });
    });
    When('the user opens Settings > Preferences', async () => {
      toggle = await openSettings(world);
    });
    Then('the "Share anonymous usage" switch is on', () => {
      expect(toggle).toHaveAttribute('aria-checked', 'true');
    });
    And('its hint says only the mod id and a random install id are sent', () => {
      expect(toggle).toHaveAccessibleDescription(/only the mod id and a random install id/);
    });
  });

  Scenario('The launcher reports a successful install', ({ Given, When, Then }) => {
    let world: World;
    Given('usage sharing is on', async () => {
      world = await launch({ stored: 'on' });
    });
    When('the user installs a catalog mod successfully', async () => {
      await act(() => world.controller.installCatalogEntry(ID));
    });
    Then('the launcher reports that mod once', () => {
      expect(world.controller.getState().catalogInstalls[ID]).toEqual({ status: 'installed', enabled: false });
      expect(world.stats.report).toHaveBeenCalledTimes(1);
      expect(world.stats.report).toHaveBeenCalledWith(ID);
    });
  });

  Scenario('Opting out', ({ Given, When, And, Then }) => {
    let world: World;
    Given('usage sharing is on', async () => {
      world = await launch({ stored: null });
    });
    When('the user turns off "Share anonymous usage" in Settings', async () => {
      const toggle = await openSettings(world);
      await world.user.click(toggle);
      expect(toggle).toHaveAttribute('aria-checked', 'false');
    });
    And('they install a mod', async () => {
      await act(() => world.controller.installCatalogEntry(ID));
    });
    Then('nothing is sent', () => {
      expect(world.controller.getState().catalogInstalls[ID]).toEqual({ status: 'installed', enabled: false });
      expect(world.stats.report).not.toHaveBeenCalled();
    });
    And('the stored usage preference is "off"', () => {
      expect(world.store.value).toBe('off');
    });
  });

  Scenario('Failed or cancelled installs are not reported', ({ Given, When, Then }) => {
    let world: World;
    Given('usage sharing is on', async () => {
      world = await launch({ stored: 'on' });
    });
    When('an install fails or is cancelled', async () => {
      world.gateway.failNext('installCatalogEntry', { code: 'download_network', message: 'offline' });
      await act(() => world.controller.installCatalogEntry(ID));
      world.gateway.failNext('installCatalogEntry', { code: 'download_cancelled', message: 'cancelled' });
      await act(() => world.controller.installCatalogEntry('tyre-model'));
    });
    Then('nothing is sent', () => {
      expect(world.gateway.callsOf('installCatalogEntry')).toHaveLength(2);
      expect(world.stats.report).not.toHaveBeenCalled();
    });
  });

  Scenario('Counts on Discover and the mod page', ({ Given, When, Then }) => {
    let world: World;
    let list: HTMLElement;
    let page: HTMLElement;
    Given('the server knows the download counts', async () => {
      world = await launch();
    });
    When('the user opens Discover and a mod page', async () => {
      list = await openDiscover(world);
      await within(list).findByText('1,234 downloads');
    });
    Then('each card and the stats strip show the downloads', async () => {
      // A mod the server never counted has no downloads yet.
      expect(within(list).getByRole('button', { name: 'Tyre Model' })).toHaveAccessibleDescription(/0 downloads/);
      expect(within(list).getByRole('button', { name: 'Rebalanced AI' })).toHaveAccessibleDescription(
        /1,234 downloads/,
      );
      expect(world.stats.totals).toHaveBeenCalledTimes(1);
      page = await openMod(world, 'Rebalanced AI');
      const strip = page.querySelector('.mod-stats') as HTMLElement;
      const cell = (await within(strip).findByText('Downloads')).closest('div') as HTMLElement;
      expect(cell).toHaveTextContent('1,234');
      expect(cell).toHaveTextContent('56 in the last 30 days');
    });
  });

  Scenario('The 30-day chart', ({ Given, When, Then, And }) => {
    let world: World;
    let chart: HTMLElement;
    Given('the server knows the daily downloads of a mod', async () => {
      world = await launch();
      await openDiscover(world);
    });
    When('the mod page opens', async () => {
      const page = await openMod(world, 'Rebalanced AI');
      chart = await within(page).findByRole('img', { name: /in the last 30 days/ });
    });
    Then('a bar chart shows one bar per day for the last 30 days', () => {
      expect(world.stats.daily).toHaveBeenCalledWith(ID);
      expect(chart.querySelectorAll('[data-day]')).toHaveLength(30);
    });
    And('each bar names its date and count', () => {
      const bars = [...chart.querySelectorAll('[data-day]')];
      const title = (index: number) => bars[index]?.querySelector('title');
      expect(title(0)).toHaveTextContent('Oct 28: 0 downloads');
      expect(title(23)).toHaveTextContent('Nov 20: 12 downloads');
      expect(title(29)).toHaveTextContent('Nov 26: 3 downloads');
      // Zero days still draw a baseline; only counted days get a filled bar.
      expect(chart.querySelectorAll('[data-day][data-empty]')).toHaveLength(28);
    });
    And('the chart is summarised in text', () => {
      expect(chart).toHaveAccessibleName('15 downloads in the last 30 days, peak of 12 on Nov 20');
    });
  });

  Scenario('Offline', ({ Given, When, Then }) => {
    let world: World;
    let page: HTMLElement;
    Given('the community server cannot be reached', async () => {
      world = await launch({ online: false, stored: 'on' });
    });
    When('the user opens Discover, a mod page or installs a mod', async () => {
      await openDiscover(world);
      await waitFor(() => expect(world.stats.totals).toHaveBeenCalled());
      await settle();
      page = await openMod(world, 'Rebalanced AI');
      await waitFor(() => expect(world.stats.daily).toHaveBeenCalled());
      await settle();
      await act(() => world.controller.installCatalogEntry(ID));
    });
    Then('no count, no chart and no error are shown', () => {
      expect(screen.queryByText(/downloads?\b/i)).not.toBeInTheDocument();
      expect(within(page).queryByRole('img', { name: /last 30 days/ })).not.toBeInTheDocument();
      expect(within(page).queryByText('Downloads')).not.toBeInTheDocument();
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      expect(world.controller.getState().error).toBeNull();
      expect(world.controller.getState().catalogInstalls[ID]).toEqual({ status: 'installed', enabled: false });
    });
  });
});
