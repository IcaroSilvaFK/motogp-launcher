// Binds tests/features/frontend/mod-page.feature (#115): the real App and the real catalog
// controller over a scripted catalog gateway. Opening a Discover card swaps the list for the mod page.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import { createCatalogController } from '../../application';
import type { CatalogGateway } from '../../application';
import type { Catalog, CatalogEntry } from '../../domain';
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

const shots = ['https://img.test/1.png', 'https://img.test/2.png', 'https://img.test/3.png'];
const SHA = 'ab'.repeat(32);
const TWO_MB = 2 * 1024 * 1024;

const rebalanced = entry({
  id: 'rebalanced-ai',
  title: 'Rebalanced AI',
  description: 'Smarter and fairer AI riders.',
  version: '2.3.1',
  updatedAt: '2026-11-10',
  screenshots: shots,
});
const tyres = entry({ id: 'tyre-model', title: 'Tyre Model', author: 'Grip Lab', updatedAt: '2026-11-20' });
const livery = entry({
  id: 'yamaha-livery',
  title: 'Classic Yamaha Livery',
  author: 'Paint Shop',
  category: 'liveries',
  updatedAt: '2026-09-01',
});

const catalogOf = (entries: CatalogEntry[]): Catalog => ({
  entries,
  fetchedAt: Date.UTC(2026, 10, 24, 12),
  stale: false,
  source: 'remote',
  skipped: 0,
});

let user: ReturnType<typeof userEvent.setup>;

async function openDiscover(entries: CatalogEntry[]) {
  const gateway: CatalogGateway = {
    getCatalog: vi.fn(() => Promise.resolve(catalogOf(entries))),
    refreshCatalog: vi.fn(() => Promise.resolve(catalogOf(entries))),
    openCatalogPage: vi.fn(() => Promise.resolve()),
  };
  user = userEvent.setup();
  render(<App controller={createFakeController()} catalog={createCatalogController({ gateway })} locale="en" />);
  await user.click(
    within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', { name: 'Discover' }),
  );
  await screen.findByRole('list', { name: /results/i });
}

async function openMod(title: string) {
  await user.click(screen.getByRole('button', { name: title }));
  return screen.getByRole('region', { name: title });
}

/** The scroll container of the content column; jsdom does not scroll, so scrollTop is a plain property. */
function scrollContainer(): HTMLElement {
  const main = document.querySelector('main') as HTMLElement;
  Object.defineProperty(main, 'scrollTop', { value: 0, writable: true, configurable: true });
  return main;
}

const stats = () => document.querySelector('.mod-stats') as HTMLElement;
const mainShot = () => screen.getByRole('button', { name: /^Rebalanced AI, screenshot \d of \d$/ });
const mainImage = () => mainShot().querySelector('img') as HTMLImageElement;
const thumbs = () => within(screen.getByRole('group', { name: 'Screenshot thumbnails' })).getAllByRole('button');

const feature = await loadFrontendFeature('mod-page');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  Scenario('Opening a mod shows its page', ({ Given, When, Then, And }) => {
    let scroller: HTMLElement;
    Given('the catalog has a mod with screenshots', async () => {
      await openDiscover([rebalanced, tyres, livery]);
      await user.click(screen.getByRole('button', { name: 'List view' }));
      await user.type(screen.getByRole('searchbox', { name: 'Search the catalog' }), 'a');
      await user.selectOptions(screen.getByRole('combobox', { name: 'Sort by' }), 'name');
    });
    When('the user opens it from Discover', async () => {
      scroller = scrollContainer();
      scroller.scrollTop = 240;
      await openMod('Rebalanced AI');
    });
    Then('a full page shows its title, author, stats, gallery and description', () => {
      const page = screen.getByRole('region', { name: 'Rebalanced AI' });
      expect(within(page).getByRole('heading', { level: 2, name: 'Rebalanced AI' })).toBeVisible();
      expect(screen.queryByRole('heading', { level: 2, name: 'Discover' })).not.toBeInTheDocument();
      expect(within(page).getByText('by Ana Rider')).toBeVisible();
      expect(within(stats()).getByText('2.3.1')).toBeVisible();
      expect(within(stats()).getByText('Nov 10, 2026')).toBeVisible();
      expect(within(page).getByRole('region', { name: 'Screenshots' })).toBeVisible();
      expect(within(page).getByText('Smarter and fairer AI riders.')).toBeVisible();
      const crumbs = within(page).getByRole('navigation', { name: 'Breadcrumb' });
      expect(within(crumbs).getByRole('button', { name: 'Discover' })).toBeVisible();
      expect(within(crumbs).getByRole('button', { name: 'physics' })).toBeVisible();
      expect(within(crumbs).getByText('Rebalanced AI')).toHaveAttribute('aria-current', 'page');
      // The page starts at its top and focus moves to Back.
      expect(scroller.scrollTop).toBe(0);
      expect(within(page).getByRole('button', { name: 'Back to Discover' })).toHaveFocus();
    });
    And('a back button returns to the same Discover list and scroll position', async () => {
      await user.click(screen.getByRole('button', { name: 'Back to Discover' }));
      expect(screen.getByRole('heading', { level: 2, name: 'Discover' })).toBeVisible();
      expect(screen.getByRole('list', { name: /results/i })).toHaveAttribute('data-view', 'list');
      expect(screen.getByRole('searchbox', { name: 'Search the catalog' })).toHaveValue('a');
      expect(screen.getByRole('combobox', { name: 'Sort by' })).toHaveValue('name');
      expect(scroller.scrollTop).toBe(240);
      expect(screen.getByRole('button', { name: 'Rebalanced AI' })).toHaveFocus();

      // Escape on the page goes back too.
      await openMod('Rebalanced AI');
      await user.keyboard('{Escape}');
      expect(screen.getByRole('button', { name: 'Rebalanced AI' })).toHaveFocus();
    });
  });

  Scenario('The breadcrumb leads back to the category', ({ Given, When, Then }) => {
    Given('the user opened a mod of the category "liveries"', async () => {
      await openDiscover([rebalanced, tyres, livery]);
      await openMod('Classic Yamaha Livery');
    });
    When('the user selects "liveries" in the breadcrumb', async () => {
      const crumbs = screen.getByRole('navigation', { name: 'Breadcrumb' });
      await user.click(within(crumbs).getByRole('button', { name: 'liveries' }));
    });
    Then('the Discover list shows only the "liveries" category', () => {
      const tiles = screen.getByRole('group', { name: 'Categories' });
      expect(within(tiles).getByRole('button', { name: /^liveries/ })).toHaveAttribute('aria-pressed', 'true');
      const cards = within(screen.getByRole('list', { name: /results/i })).getAllByRole('button');
      expect(cards).toHaveLength(1);
      expect(cards[0]).toHaveAccessibleName('Classic Yamaha Livery');
      expect(cards[0]).toHaveFocus();
    });
  });

  Scenario('Browsing screenshots', ({ Given, When, Then, And }) => {
    Given('the mod page shows 3 screenshots', async () => {
      await openDiscover([rebalanced]);
      await openMod('Rebalanced AI');
      expect(thumbs()).toHaveLength(3);
    });
    When('the user selects the second thumbnail', async () => {
      await user.click(thumbs()[1] as HTMLElement);
    });
    Then('it becomes the main image', () => {
      expect(thumbs()[1]).toHaveAttribute('aria-pressed', 'true');
      expect(mainImage()).toHaveAttribute('src', shots[1]);
      expect(mainShot()).toHaveAccessibleName('Rebalanced AI, screenshot 2 of 3');
    });
    And('pressing Enter opens it in a lightbox that Escape closes', async () => {
      mainShot().focus();
      await user.keyboard('{Enter}');
      const dialog = screen.getByRole('dialog', { name: 'Rebalanced AI screenshots' });
      expect(within(dialog).getByRole('img')).toHaveAttribute('src', shots[1]);
      expect(within(dialog).getByText('2 / 3')).toBeInTheDocument();
      await user.keyboard('{Escape}');
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
      // Escape closed only the lightbox: the mod page stays and focus is back on the main image.
      expect(screen.getByRole('region', { name: 'Rebalanced AI' })).toBeInTheDocument();
      expect(mainShot()).toHaveFocus();
    });
  });

  Scenario('A broken screenshot collapses', ({ Given, Then }) => {
    Given('the mod page shows 2 screenshots and the first fails to load', async () => {
      await openDiscover([{ ...rebalanced, screenshots: shots.slice(0, 2) }]);
      await openMod('Rebalanced AI');
      fireEvent.error(thumbs()[0]?.querySelector('img') as HTMLImageElement);
    });
    Then('its thumbnail is removed and the second becomes the main image', () => {
      expect(screen.queryByRole('group', { name: 'Screenshot thumbnails' })).not.toBeInTheDocument();
      expect(mainImage()).toHaveAttribute('src', shots[1]);
      expect(document.querySelector(`img[src="${shots[0]}"]`)).toBeNull();
    });
  });

  Scenario('A mod without screenshots', ({ Given, When, Then }) => {
    Given('a mod has no screenshots', async () => {
      await openDiscover([tyres]);
    });
    When('its page opens', async () => {
      await openMod('Tyre Model');
    });
    Then('no gallery is shown and nothing shifts', () => {
      const page = screen.getByRole('region', { name: 'Tyre Model' });
      expect(within(page).queryByRole('region', { name: 'Screenshots' })).not.toBeInTheDocument();
      // No reserved box or skeleton is left where the gallery would be.
      expect(page.querySelector('.gallery, .gallery-stage, .skeleton')).toBeNull();
      expect(page.querySelector('img')).toBeNull();
    });
  });

  Scenario('Unsafe content is not rendered', ({ Given, Then }) => {
    const html = '<script>alert(1)</script><img src="x" onerror="alert(2)"><a href="https://x.test">x</a>';
    Given('a long description contains raw HTML', async () => {
      await openDiscover([{ ...tyres, longDescription: html }]);
      await openMod('Tyre Model');
    });
    Then('the HTML is shown as text, not rendered', () => {
      const description = screen.getByRole('region', { name: 'Description' });
      expect(description).toHaveTextContent(html);
      expect(description.querySelector('script, img, a')).toBeNull();
    });
  });

  Scenario('The long description uses the safe Markdown subset', ({ Given, Then }) => {
    Given('a long description with a heading, bold text and a list', async () => {
      await openDiscover([{ ...tyres, longDescription: '# Features\n\nA **new** tyre model.\n\n- Grip\n- Wear' }]);
      await openMod('Tyre Model');
    });
    Then('the page shows a heading, bold text and a list', () => {
      const description = screen.getByRole('region', { name: 'Description' });
      expect(within(description).getByRole('heading', { level: 3, name: 'Features' })).toBeVisible();
      expect(description.querySelector('strong')).toHaveTextContent('new');
      expect(
        within(description)
          .getAllByRole('listitem')
          .map((item) => item.textContent),
      ).toEqual(['Grip', 'Wear']);
      // The long description replaces the short one.
      expect(within(description).queryByText('A mod.')).not.toBeInTheDocument();
    });
  });

  Scenario('Download stats appear only for downloadable mods', ({ Given, Then, And }) => {
    Given('a mod with a download of 2 MB', async () => {
      const download = { url: 'https://github.com/x/y.zip', size: TWO_MB, sha256: SHA };
      await openDiscover([{ ...tyres, download }, livery]);
      await openMod('Tyre Model');
    });
    Then('the stats show its size and SHA-256', () => {
      expect(within(stats()).getByText('Size')).toBeVisible();
      expect(within(stats()).getByText('2 MB')).toBeVisible();
      expect(within(stats()).getByText('SHA-256')).toBeVisible();
      expect(within(stats()).getByText(SHA)).toHaveAttribute('title', SHA);
    });
    And('a mod without a download shows no size or SHA-256', async () => {
      await user.click(screen.getByRole('button', { name: 'Back to Discover' }));
      await openMod('Classic Yamaha Livery');
      expect(within(stats()).getByText('Version')).toBeVisible();
      expect(within(stats()).queryByText('Size')).not.toBeInTheDocument();
      expect(within(stats()).queryByText('SHA-256')).not.toBeInTheDocument();
    });
  });
});
