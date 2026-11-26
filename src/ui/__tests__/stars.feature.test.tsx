// Binds tests/features/frontend/stars.feature (#119): the real App, the real catalog controller over a
// scripted gateway, and in-memory fakes of the CommunityAccount and ModStatsSource ports that behave
// like the backend (the token stays on their side; signed out, a star is refused).
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import { createCatalogController } from '../../application';
import type { CatalogGateway, CommunityAccount, ModStatsSource } from '../../application';
import { ModStatsError } from '../../domain';
import type { Catalog, CatalogEntry, CommunityStatus, ModStats } from '../../domain';
import { App } from '../App';
import { createFakeController } from './fakeController';
import { loadFrontendFeature } from './features';

function entry(patch: Partial<CatalogEntry> & { id: string; title: string }): CatalogEntry {
  return {
    author: 'Ana Rider',
    game: 'MotoGP 25',
    category: 'physics',
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

const rebalanced = entry({ id: 'rebalanced-ai', title: 'Rebalanced AI', updatedAt: '2026-11-20' });
const tyres = entry({ id: 'tyre-model', title: 'Tyre Model', updatedAt: '2026-11-10' });
const livery = entry({ id: 'yamaha-livery', title: 'Classic Yamaha Livery', updatedAt: '2026-09-01' });

const catalogOf = (entries: CatalogEntry[]): Catalog => ({
  entries,
  fetchedAt: Date.UTC(2026, 10, 24, 12),
  stale: false,
  source: 'remote',
  skipped: 0,
});

/** The account as the backend reports it; `signIn()` waits until the test authorizes in the browser. */
class FakeAccount implements CommunityAccount {
  private authorize: ((status: CommunityStatus) => void) | null = null;
  constructor(public current: CommunityStatus) {}
  status = () => Promise.resolve(this.current);
  signIn = () =>
    new Promise<CommunityStatus>((resolve) => {
      this.authorize = resolve;
    });
  signOut = () => Promise.resolve();
  authorizeInBrowser() {
    this.current = { state: 'signed_in', profile: { displayName: 'rider', avatarDataUrl: null } };
    this.authorize?.(this.current);
  }
}

/** The server's stars, one per (user, mod); the fake knows only one user. */
class FakeStars implements ModStatsSource {
  readonly stars = new Map<string, number>();
  readonly mine = new Set<string>();
  offline = false;
  constructor(private readonly account: FakeAccount) {}

  private view(id: string): ModStats {
    return { id, stars: this.stars.get(id) ?? 0, starredByMe: this.mine.has(id) };
  }

  stats = vi.fn(async (): Promise<ModStats[]> => {
    if (this.offline) return [];
    const signedIn = this.account.current.state === 'signed_in';
    return [...this.stars.keys()].map((id) => (signedIn ? this.view(id) : { id, stars: this.stars.get(id) ?? 0 }));
  });

  private change(id: string, star: boolean): Promise<ModStats> {
    if (this.offline) return Promise.reject(new ModStatsError('offline'));
    if (this.account.current.state !== 'signed_in') return Promise.reject(new ModStatsError('signedOut'));
    if (star !== this.mine.has(id)) {
      this.stars.set(id, (this.stars.get(id) ?? 0) + (star ? 1 : -1));
      if (star) this.mine.add(id);
      else this.mine.delete(id);
    }
    return Promise.resolve(this.view(id));
  }

  star = vi.fn((id: string) => this.change(id, true));
  unstar = vi.fn((id: string) => this.change(id, false));
}

interface World {
  account: FakeAccount;
  stars: FakeStars;
  user: ReturnType<typeof userEvent.setup>;
}

const SIGNED_IN: CommunityStatus = { state: 'signed_in', profile: { displayName: 'rider', avatarDataUrl: null } };

function setUp(status: CommunityStatus, counts: Record<string, number>, mine: string[] = []): World {
  const account = new FakeAccount(status);
  const stars = new FakeStars(account);
  for (const [id, count] of Object.entries(counts)) stars.stars.set(id, count);
  for (const id of mine) stars.mine.add(id);
  return { account, stars, user: userEvent.setup() };
}

async function openDiscover(world: World) {
  const entries = [rebalanced, tyres, livery];
  const gateway: CatalogGateway = {
    getCatalog: vi.fn(() => Promise.resolve(catalogOf(entries))),
    refreshCatalog: vi.fn(() => Promise.resolve(catalogOf(entries))),
    openCatalogPage: vi.fn(() => Promise.resolve()),
  };
  render(
    <App
      controller={createFakeController()}
      catalog={createCatalogController({ gateway })}
      communityAccount={world.account}
      modStats={world.stars}
      locale="en"
    />,
  );
  await world.user.click(
    within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', { name: 'Discover' }),
  );
  await screen.findByRole('list', { name: /results/i });
}

async function openMod(world: World, title: string) {
  await world.user.click(screen.getByRole('button', { name: title }));
  return screen.getByRole('region', { name: title });
}

const starButton = () => screen.findByRole('button', { name: 'Star' });

const feature = await loadFrontendFeature('stars');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  Scenario('Starring a mod', ({ Given, When, Then }) => {
    let world: World;
    Given('the user is signed in', async () => {
      world = setUp(SIGNED_IN, { 'rebalanced-ai': 10 });
      await openDiscover(world);
    });
    When('they star a mod with 10 stars', async () => {
      const page = await openMod(world, 'Rebalanced AI');
      expect(await within(page).findByText('10 stars')).toBeInTheDocument();
      const button = await starButton();
      expect(button).toHaveAttribute('aria-pressed', 'false');
      await world.user.click(button);
    });
    Then('it shows 11 and the button shows as starred', async () => {
      expect(await screen.findByText('11 stars')).toBeInTheDocument();
      expect(await starButton()).toHaveAttribute('aria-pressed', 'true');
      expect(world.stars.star).toHaveBeenCalledWith('rebalanced-ai');
      expect(world.stars.stars.get('rebalanced-ai')).toBe(11);
    });
  });

  Scenario('Unstarring a mod', ({ Given, When, Then }) => {
    let world: World;
    Given('the user is signed in and has starred a mod with 11 stars', async () => {
      world = setUp(SIGNED_IN, { 'rebalanced-ai': 11 }, ['rebalanced-ai']);
      await openDiscover(world);
      await openMod(world, 'Rebalanced AI');
      expect(await starButton()).toHaveAttribute('aria-pressed', 'true');
    });
    When('they choose the star button again', async () => {
      await world.user.click(await starButton());
    });
    Then('it shows 10 and the button shows as not starred', async () => {
      expect(await screen.findByText('10 stars')).toBeInTheDocument();
      expect(await starButton()).toHaveAttribute('aria-pressed', 'false');
      expect(world.stars.unstar).toHaveBeenCalledWith('rebalanced-ai');
    });
  });

  Scenario('Starring needs an account', ({ Given, When, Then }) => {
    let world: World;
    Given('the user is signed out', async () => {
      world = setUp({ state: 'signed_out' }, { 'rebalanced-ai': 4 });
      await openDiscover(world);
      await openMod(world, 'Rebalanced AI');
    });
    When('they choose the star button', async () => {
      await world.user.click(await starButton());
    });
    Then('they are offered "Sign in with Discord"', async () => {
      const prompt = await screen.findByRole('group', { name: 'Sign in with Discord to star mods.' });
      const signIn = within(prompt).getByRole('button', { name: 'Sign in with Discord' });
      expect(signIn).toHaveFocus();
      // Nothing was starred and the count did not move.
      expect(world.stars.star).not.toHaveBeenCalled();
      expect(screen.getByText('4 stars')).toBeInTheDocument();
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      // Esc closes the prompt without leaving the mod page, and focus returns to the star.
      await world.user.keyboard('{Escape}');
      expect(screen.queryByRole('group', { name: 'Sign in with Discord to star mods.' })).not.toBeInTheDocument();
      expect(await starButton()).toHaveFocus();
      expect(screen.getByRole('region', { name: 'Rebalanced AI' })).toBeInTheDocument();
    });
  });

  Scenario('Signing in from the star applies it', ({ Given, When, Then }) => {
    let world: World;
    Given('the user is signed out and was offered "Sign in with Discord" on a mod with 4 stars', async () => {
      world = setUp({ state: 'signed_out' }, { 'rebalanced-ai': 4 });
      await openDiscover(world);
      await openMod(world, 'Rebalanced AI');
      await world.user.click(await starButton());
    });
    When('they sign in with Discord', async () => {
      const prompt = await screen.findByRole('group', { name: 'Sign in with Discord to star mods.' });
      await world.user.click(within(prompt).getByRole('button', { name: 'Sign in with Discord' }));
      expect(within(prompt).getByRole('status')).toHaveTextContent('Waiting for the browser…');
      world.account.authorizeInBrowser();
    });
    Then('the mod is starred and shows 5', async () => {
      await waitFor(() => expect(world.stars.star).toHaveBeenCalledWith('rebalanced-ai'));
      expect(await screen.findByText('5 stars')).toBeInTheDocument();
      expect(await starButton()).toHaveAttribute('aria-pressed', 'true');
      expect(screen.queryByRole('group', { name: 'Sign in with Discord to star mods.' })).not.toBeInTheDocument();
    });
  });

  Scenario('Sort by stars', ({ Given, When, Then }) => {
    let world: World;
    Given('mods with 5, 12 and 0 stars', async () => {
      world = setUp({ state: 'signed_out' }, { 'rebalanced-ai': 5, 'yamaha-livery': 12 });
      await openDiscover(world);
      await screen.findByText('12 stars');
    });
    When('the user sorts Discover by most stars', async () => {
      await world.user.selectOptions(screen.getByRole('combobox', { name: 'Sort by' }), 'Most stars');
    });
    Then('the mods are listed with 12, 5 and 0 stars', () => {
      const cards = within(screen.getByRole('list', { name: /results/i })).getAllByRole('button');
      expect(cards.map((card) => card.textContent)).toEqual([
        expect.stringContaining('Classic Yamaha Livery'),
        expect.stringContaining('Rebalanced AI'),
        expect.stringContaining('Tyre Model'),
      ]);
      expect(within(cards[0] as HTMLElement).getByText('12 stars')).toBeInTheDocument();
      expect(within(cards[1] as HTMLElement).getByText('5 stars')).toBeInTheDocument();
      expect(within(cards[2] as HTMLElement).getByText('0 stars')).toBeInTheDocument();
    });
  });

  Scenario('Offline community', ({ Given, When, Then }) => {
    let world: World;
    Given('the community server is unreachable', () => {
      world = setUp({ state: 'offline' }, { 'rebalanced-ai': 5 });
      world.stars.offline = true;
    });
    When('the user opens Discover', async () => {
      await openDiscover(world);
      await waitFor(() => expect(world.stars.stats).toHaveBeenCalled());
    });
    Then('the mods are listed without star counts and no error is shown', async () => {
      const list = screen.getByRole('list', { name: /results/i });
      expect(within(list).getAllByRole('button')).toHaveLength(3);
      expect(within(list).queryByText(/stars?$/)).not.toBeInTheDocument();
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      // The mod page has no star button either.
      await openMod(world, 'Rebalanced AI');
      expect(screen.queryByRole('button', { name: 'Star' })).not.toBeInTheDocument();
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });
  });

  Scenario('Stats are cached briefly', ({ Given, When, Then }) => {
    let world: World;
    Given('Discover loaded the stats less than a minute ago', async () => {
      world = setUp(SIGNED_IN, { 'rebalanced-ai': 7 });
      await openDiscover(world);
      await screen.findByText('7 stars');
      expect(world.stars.stats).toHaveBeenCalledTimes(1);
    });
    When('the user opens a mod page', async () => {
      await openMod(world, 'Rebalanced AI');
      await starButton();
    });
    Then('the stats are not fetched again', () => {
      expect(world.stars.stats).toHaveBeenCalledTimes(1);
      expect(screen.getByText('7 stars')).toBeInTheDocument();
    });
  });
});
