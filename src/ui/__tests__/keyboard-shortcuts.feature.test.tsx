// Binds tests/features/frontend/keyboard-shortcuts.feature (#157): the real App, the real catalog
// controller over a scripted gateway, and real key events on the document.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import { createCatalogController } from '../../application';
import type { CatalogGateway } from '../../application';
import type { Catalog, CatalogEntry } from '../../domain';
import { App } from '../App';
import { createFakeController } from './fakeController';
import { loadFrontendFeature } from './features';

const mod: CatalogEntry = {
  id: 'rebalanced-ai',
  title: 'Rebalanced AI',
  author: 'Ana Rider',
  game: 'MotoGP 25',
  category: 'physics',
  description: 'Smarter and fairer AI riders.',
  version: '1.0.0',
  updatedAt: '2026-01-01',
  pageUrl: 'https://overtake.gg/downloads/rebalanced-ai/',
  thumbnailUrl: null,
  screenshots: [],
  longDescription: null,
  download: null,
};

const catalog: Catalog = {
  entries: [mod],
  fetchedAt: Date.UTC(2026, 10, 24, 12),
  stale: false,
  source: 'remote',
  skipped: 0,
};

const game = { name: 'MotoGP 25', root: 'D:/Steam/MotoGP25', mods_dir: 'D:/Steam/MotoGP25/~mods', app_id: 1234 };

let user: ReturnType<typeof userEvent.setup>;

async function goTo(name: string) {
  await user.click(within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', { name }));
}

async function openApp(screenName?: string) {
  const gateway: CatalogGateway = {
    getCatalog: vi.fn(() => Promise.resolve(catalog)),
    refreshCatalog: vi.fn(() => Promise.resolve(catalog)),
    openCatalogPage: vi.fn(() => Promise.resolve()),
  };
  user = userEvent.setup();
  render(
    <App
      controller={createFakeController({
        game,
        mods: [{ id: 'fast', name: 'Fast Bike', files: ['a.pak'], size_bytes: 1 }],
      })}
      catalog={createCatalogController({ gateway })}
      locale="en"
    />,
  );
  if (screenName) await goTo(screenName);
}

const librarySearch = () => screen.getByRole('searchbox', { name: /search/i });

const feature = await loadFrontendFeature('keyboard-shortcuts');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  Scenario('Esc leaves the mod page', ({ Given, When, Then }) => {
    Given('the user opened a mod page from Discover and focus is on the page body', async () => {
      await openApp('Discover');
      await user.click(await screen.findByRole('button', { name: 'Rebalanced AI' }));
      expect(screen.getByRole('region', { name: 'Rebalanced AI' })).toBeVisible();
      (document.activeElement as HTMLElement).blur();
    });
    When('they press Esc', async () => {
      await user.keyboard('{Escape}');
    });
    Then('Discover is shown with the mod card focused', () => {
      expect(screen.getByRole('heading', { level: 2, name: 'Discover' })).toBeVisible();
      expect(screen.getByRole('button', { name: 'Rebalanced AI' })).toHaveFocus();
    });
  });

  Scenario('Slash focuses search', ({ Given, When, Then }) => {
    Given('the Library is open', async () => {
      await openApp();
      await screen.findByRole('heading', { level: 2, name: 'Library' });
    });
    When('the user presses "/"', async () => {
      await user.keyboard('/');
    });
    Then('the Library search field is focused', () => {
      expect(librarySearch()).toHaveFocus();
      expect(librarySearch()).toHaveValue('');
    });
  });

  Scenario('Ctrl+F focuses search', ({ Given, When, Then }) => {
    Given('Discover is open', async () => {
      await openApp('Discover');
      await screen.findByRole('button', { name: 'Rebalanced AI' });
    });
    When('the user presses Ctrl+F', async () => {
      await user.keyboard('{Control>}f{/Control}');
    });
    Then('the Discover search field is focused', () => {
      expect(screen.getByRole('searchbox', { name: 'Search the catalog' })).toHaveFocus();
    });
  });

  Scenario('Shortcuts are ignored while typing', ({ Given, When, Then }) => {
    Given('the focus is in the Library search field', async () => {
      await openApp();
      await user.click(await screen.findByRole('searchbox', { name: /search/i }));
    });
    When('the user types "/"', async () => {
      await user.keyboard('a/b');
    });
    Then('a "/" is typed and the focus stays put', () => {
      expect(librarySearch()).toHaveValue('a/b');
      expect(librarySearch()).toHaveFocus();
    });
  });

  Scenario('Esc is ignored while typing on the mod page', ({ Given, When, Then }) => {
    Given('a mod page is open and focus is in a text field', async () => {
      await openApp('Discover');
      await user.click(await screen.findByRole('button', { name: 'Rebalanced AI' }));
      const field = document.createElement('input');
      document.body.appendChild(field);
      field.focus();
    });
    When('the user presses Esc', async () => {
      await user.keyboard('{Escape}');
    });
    Then('the mod page stays open', () => {
      expect(screen.getByRole('region', { name: 'Rebalanced AI' })).toBeVisible();
    });
  });

  Scenario('Switching sections', ({ Given, When, Then }) => {
    Given('the Library is open', async () => {
      await openApp();
      await screen.findByRole('heading', { level: 2, name: 'Library' });
    });
    When('the user presses Ctrl+2', async () => {
      await user.keyboard('{Control>}2{/Control}');
    });
    Then('the second sidebar section opens', async () => {
      expect(await screen.findByRole('heading', { level: 2, name: 'Discover' })).toBeVisible();
      expect(
        within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', { name: 'Discover' }),
      ).toHaveAttribute('aria-current', 'page');
    });
  });

  Scenario('Settings lists the shortcuts', ({ Given, Then }) => {
    Given('the Settings screen is open', async () => {
      await openApp('Settings');
    });
    Then('the keyboard shortcuts are listed in About', () => {
      expect(screen.getByText('Keyboard shortcuts')).toBeVisible();
      expect(screen.getByText('Esc')).toBeVisible();
      expect(screen.getByText('Ctrl+1…4')).toBeVisible();
    });
  });
});
