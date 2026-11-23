// Binds tests/features/frontend/ui-style.feature: the real UI over a fake controller, plus
// static checks of the design tokens read from styles.css.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { readFileSync } from 'node:fs';
import { expect } from 'vitest';
import type { LauncherState } from '../../application';
import type { ModEntry } from '../../domain';
import { App } from '../App';
import { createFakeController } from './fakeController';
import type { FakeController } from './fakeController';
import { loadFrontendFeature } from './features';

const modA: ModEntry = { id: 'A', name: 'Mod A', files: ['a.pak', 'a.utoc'], size_bytes: 2048 };
const modB: ModEntry = { id: 'B', name: 'Mod B', files: ['b.pak'], size_bytes: 500 };
const game = { name: 'MotoGP 25', root: 'D:/Steam/MotoGP25', mods_dir: 'D:/Steam/MotoGP25/~mods', app_id: 1234 };

const css = readFileSync(`${import.meta.dirname}/../styles.css`, 'utf8');

/** Custom properties declared in the first :root block. */
function rootTokens(source: string): Map<string, string> {
  const block = /:root\s*\{([^}]*)\}/.exec(source)?.[1] ?? '';
  const tokens = new Map<string, string>();
  for (const [, name = '', value = ''] of block.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) tokens.set(name, value.trim());
  return tokens;
}

function channel(value: number): number {
  const c = value / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  const [r = 0, g = 0, b = 0] = [1, 3, 5].map((i) => channel(parseInt(hex.slice(i, i + 2), 16)));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG 2.x contrast ratio between two #rrggbb colors. */
function contrast(a: string, b: string): number {
  const [hi = 0, lo = 0] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const token = (name: string): string => {
  const value = rootTokens(css).get(name);
  expect(value, `token ${name}`).toMatch(/^#[0-9a-fA-F]{6}$/);
  return value as string;
};

let controller: FakeController;
let user: ReturnType<typeof userEvent.setup>;

function launch(patch: Partial<LauncherState> = {}) {
  controller = createFakeController(patch);
  user = userEvent.setup();
  render(<App controller={controller} locale="en" />);
}

const nav = () => within(screen.getByRole('navigation', { name: 'Main navigation' }));
const statusBar = () => screen.getByRole('contentinfo');
const libraryRows = () => within(screen.getByRole('list', { name: 'Library' })).getAllByRole('listitem');
const facet = (name: RegExp) => within(screen.getByRole('group', { name: 'Filter' })).getByRole('button', { name });

const twoModsOneActive: Partial<LauncherState> = {
  mods: [modA, modB],
  profiles: [
    { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
    { id: 'p1', name: 'Mine', mod_ids: ['A'] },
  ],
  config: { game_dir: null, active_profile: 'p1' },
};

const feature = await loadFrontendFeature('ui-style');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  Scenario('Play is the prominent call to action', ({ Given, Then, And }) => {
    Given('the launcher with a configured game and an active profile', () => {
      launch({ game, config: { game_dir: game.root, active_profile: 'vanilla' } });
    });
    Then('I see a "Play" button styled as the primary call to action', () => {
      expect(screen.getByRole('button', { name: 'Play' })).toHaveClass('play');
    });
    And('it is enabled', () => {
      expect(screen.getByRole('button', { name: 'Play' })).toBeEnabled();
    });
  });

  Scenario('The status bar shows the game, the session and Play', ({ Given, Then, And }) => {
    Given('the launcher with a configured game and an active profile', () => {
      launch({ game, config: { game_dir: game.root, active_profile: 'vanilla' } });
    });
    Then('the status bar shows the game name and its store', () => {
      const bar = within(statusBar());
      expect(bar.getByText('MotoGP 25')).toBeInTheDocument();
      expect(bar.getByText('Steam')).toHaveClass('badge');
    });
    And('the status bar holds the active profile selector and the "Play" button', () => {
      const bar = within(statusBar());
      expect(bar.getByLabelText('Active profile')).toHaveValue('vanilla');
      expect(bar.getByRole('button', { name: 'Play' })).toBeInTheDocument();
      expect(within(screen.getByRole('banner')).queryByRole('button', { name: 'Play' })).not.toBeInTheDocument();
    });
    And('the "Play" button is styled as the primary button', () => {
      const play = screen.getByRole('button', { name: 'Play' });
      expect(play).toHaveClass('btn', 'btn-primary', 'play');
      expect(play).toHaveAttribute('type', 'button');
    });
    And('the status bar says the launcher is ready to play', () => {
      expect(within(statusBar()).getByText('Ready to play')).toBeInTheDocument();
      // The idle message is plain text: only real session changes are announced.
      expect(screen.queryByRole('status')).not.toBeInTheDocument();
    });
  });

  Scenario('The status bar guides the setup when no game is configured', ({ Given, When, Then }) => {
    Given('the launcher without a configured game', () => {
      launch();
    });
    Then('the status bar says no game is selected', () => {
      expect(within(statusBar()).getByText('No game selected')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Play' })).toBeDisabled();
    });
    When('I click "Set up game" in the status bar', async () => {
      await user.click(within(statusBar()).getByRole('button', { name: 'Set up game' }));
    });
    Then('the Settings item is marked as the current page', () => {
      expect(nav().getByRole('button', { name: 'Settings' })).toHaveAttribute('aria-current', 'page');
      expect(screen.getByRole('heading', { level: 2, name: 'Settings' })).toBeInTheDocument();
    });
  });

  Scenario('Top navigation marks the current screen', ({ Given, When, Then, And }) => {
    Given('the launcher open on the Library', () => {
      launch();
      expect(nav().getByRole('button', { name: 'Library' })).toHaveAttribute('aria-current', 'page');
    });
    Then('the main navigation sits in the top bar with one tab per screen', () => {
      const topNav = within(screen.getByRole('banner')).getByRole('navigation', { name: 'Main navigation' });
      expect(
        within(topNav)
          .getAllByRole('button')
          .map((tab) => tab.textContent),
      ).toEqual(['Library', 'Profiles', 'Settings']);
    });
    When('I select Settings in the navigation', async () => {
      await user.click(nav().getByRole('button', { name: 'Settings' }));
    });
    Then('the Settings item is marked as the current page', () => {
      expect(nav().getByRole('button', { name: 'Settings' })).toHaveAttribute('aria-current', 'page');
    });
    And('the Library item is no longer marked', () => {
      expect(nav().getByRole('button', { name: 'Library' })).not.toHaveAttribute('aria-current');
    });
  });

  Scenario('Every screen has one visible title and a live subtitle', ({ Given, When, Then }) => {
    Given('mods A and B in the library and only A in the active profile', () => {
      launch(twoModsOneActive);
    });
    Then('the Library title is visible with the subtitle "2 mods · 1 in Mine"', () => {
      const title = screen.getByRole('heading', { level: 2, name: 'Library' });
      expect(title).not.toHaveClass('visually-hidden');
      expect(screen.getAllByRole('heading', { level: 2 })).toHaveLength(1);
      expect(screen.getByText('2 mods · 1 in Mine')).toHaveClass('page-subtitle');
    });
    When('I open the Profiles screen', async () => {
      await user.click(nav().getByRole('button', { name: 'Profiles' }));
    });
    Then('the Profiles title is visible with the subtitle "2 profiles · active: Mine"', () => {
      const title = screen.getByRole('heading', { level: 2, name: 'Profiles' });
      expect(title).not.toHaveClass('visually-hidden');
      expect(screen.getByText('2 profiles · active: Mine')).toHaveClass('page-subtitle');
    });
  });

  Scenario('The library lists mods as dense rows', ({ Given, When, Then, And }) => {
    Given('mods A and B in the library and only A in the active profile', () => {
      launch({
        mods: [modA, modB],
        profiles: [
          { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
          { id: 'p1', name: 'Mine', mod_ids: ['A'] },
        ],
        config: { game_dir: null, active_profile: 'p1' },
      });
    });
    When('I open the Library', async () => {
      await user.click(nav().getByRole('button', { name: 'Library' }));
    });
    Then('I see a list with one row per mod showing its name, size and file count', () => {
      const [rowA, rowB, ...rest] = within(screen.getByRole('list', { name: 'Library' })).getAllByRole(
        'listitem',
      ) as HTMLElement[];
      expect(rest).toHaveLength(0);
      if (!rowA || !rowB) throw new Error('expected two rows');
      expect(within(rowA).getByText('Mod A')).toBeInTheDocument();
      expect(within(rowA).getByText('2 KB')).toBeInTheDocument();
      expect(within(rowA).getByText('2 files')).toBeInTheDocument();
      expect(within(rowB).getByText('Mod B')).toBeInTheDocument();
      expect(within(rowB).getByText('1 file')).toBeInTheDocument();
    });
    And('the row for A says it is in the active profile', () => {
      const row = screen.getByText('Mod A').closest('li') as HTMLElement;
      expect(within(row).getByText('In active profile')).toBeInTheDocument();
    });
    And('the row for B says it is not in the active profile', () => {
      const row = screen.getByText('Mod B').closest('li') as HTMLElement;
      expect(within(row).getByText('Not in active profile')).toBeInTheDocument();
    });
    And('each row offers a remove button named after the mod', async () => {
      expect(screen.getByRole('button', { name: 'Remove Mod A' })).toBeInTheDocument();
      await user.click(screen.getByRole('button', { name: 'Remove Mod B' }));
      expect(controller.removeMod).toHaveBeenCalledWith('B');
    });
  });

  Scenario('Filter the library by name', ({ Given, When, Then }) => {
    Given('mods "Yamaha Livery" and "Ducati Sound" in the library', () => {
      launch({
        mods: [
          { id: 'Y', name: 'Yamaha Livery', files: ['y.pak'], size_bytes: 100 },
          { id: 'D', name: 'Ducati Sound', files: ['d.pak'], size_bytes: 100 },
        ],
      });
    });
    When('I type "duc" in the library search', async () => {
      await user.type(screen.getByRole('searchbox', { name: 'Search mods' }), 'duc');
    });
    Then('only "Ducati Sound" is listed', () => {
      const rows = within(screen.getByRole('list', { name: 'Library' })).getAllByRole('listitem');
      expect(rows).toHaveLength(1);
      expect(within(rows[0] as HTMLElement).getByText('Ducati Sound')).toBeInTheDocument();
    });
    When('I type "zzz" in the library search', async () => {
      const box = screen.getByRole('searchbox', { name: 'Search mods' });
      await user.clear(box);
      await user.type(box, 'zzz');
    });
    Then('I see the no-results message', () => {
      expect(screen.getByText('No mods match your search.')).toBeInTheDocument();
      expect(screen.queryByRole('list', { name: 'Library' })).not.toBeInTheDocument();
    });
  });

  Scenario('Empty library offers the next action', ({ Given, Then, And }) => {
    Given('an empty library', () => {
      launch({ mods: [] });
    });
    Then('I see the empty-library message', () => {
      expect(screen.getByText('No mods yet. Import some to get started.')).toBeInTheDocument();
    });
    And('an "Import mods" button that opens the import dialog', async () => {
      await user.click(screen.getByRole('button', { name: 'Import mods' }));
      expect(controller.importFromDialog).toHaveBeenCalledTimes(1);
    });
  });

  Scenario('Filter the library by active profile with counts', ({ Given, When, Then, And }) => {
    Given('mods A and B in the library and only A in the active profile', () => {
      launch(twoModsOneActive);
    });
    Then('I see the facets "All" 2, "In active profile" 1 and "Not in active profile" 1', () => {
      expect(within(facet(/^All/)).getByText('2')).toHaveClass('facet-count');
      expect(within(facet(/^In active profile/)).getByText('1')).toHaveClass('facet-count');
      expect(within(facet(/^Not in active profile/)).getByText('1')).toHaveClass('facet-count');
    });
    And('the "All" facet is selected', () => {
      expect(facet(/^All/)).toHaveAttribute('aria-pressed', 'true');
      expect(facet(/^In active profile/)).toHaveAttribute('aria-pressed', 'false');
    });
    When('I choose the "Not in active profile" facet', async () => {
      await user.click(facet(/^Not in active profile/));
    });
    Then('only "Mod B" is listed', () => {
      const rows = libraryRows();
      expect(rows).toHaveLength(1);
      expect(within(rows[0] as HTMLElement).getByText('Mod B')).toBeInTheDocument();
      expect(facet(/^Not in active profile/)).toHaveAttribute('aria-pressed', 'true');
    });
    When('I type "Mod A" in the library search', async () => {
      await user.type(screen.getByRole('searchbox', { name: 'Search mods' }), 'Mod A');
    });
    Then('I see the no-results message', () => {
      expect(screen.getByText('No mods match your search.')).toBeInTheDocument();
    });
    When('I clear the filters', async () => {
      await user.click(screen.getByRole('button', { name: 'Clear filters' }));
    });
    Then('both mods are listed again and the "All" facet is selected', () => {
      expect(libraryRows()).toHaveLength(2);
      expect(screen.getByRole('searchbox', { name: 'Search mods' })).toHaveValue('');
      expect(facet(/^All/)).toHaveAttribute('aria-pressed', 'true');
    });
  });

  Scenario('Loading shows a skeleton instead of an empty screen', ({ Given, Then, And }) => {
    Given('the launcher is still loading', () => {
      launch({ status: 'loading' });
    });
    Then('the content area is marked busy and announces that it is loading', () => {
      const main = screen.getByRole('main');
      expect(main).toHaveAttribute('aria-busy', 'true');
      expect(within(main).getByText('Loading…')).toBeInTheDocument();
      expect(main.querySelectorAll('.skeleton-row').length).toBeGreaterThan(0);
    });
    And('the empty-library message is not shown', () => {
      expect(screen.queryByText('No mods yet. Import some to get started.')).not.toBeInTheDocument();
    });
  });

  Scenario('Profiles show their mod counts and guide the selection', ({ Given, When, Then, And }) => {
    Given('mods A and B in the library and only A in the active profile', () => {
      launch(twoModsOneActive);
    });
    When('I open the Profiles screen', async () => {
      await user.click(nav().getByRole('button', { name: 'Profiles' }));
    });
    Then('each profile in the list shows its mod count', () => {
      const list = within(screen.getByRole('list', { name: 'Your profiles' }));
      const vanilla = list.getByRole('button', { name: 'Vanilla' }).closest('li') as HTMLElement;
      const mine = list.getByRole('button', { name: 'Mine' }).closest('li') as HTMLElement;
      expect(within(vanilla).getByText('0 mods')).toBeInTheDocument();
      expect(within(mine).getByText('1 mod')).toBeInTheDocument();
    });
    And('the active profile is marked as active', () => {
      const list = within(screen.getByRole('list', { name: 'Your profiles' }));
      const mine = list.getByRole('button', { name: 'Mine' }).closest('li') as HTMLElement;
      const vanilla = list.getByRole('button', { name: 'Vanilla' }).closest('li') as HTMLElement;
      expect(within(mine).getByText('Active')).toHaveClass('badge');
      expect(within(vanilla).queryByText('Active')).not.toBeInTheDocument();
    });
    And('the editor area asks me to select a profile', () => {
      expect(screen.getByText('Select a profile to edit its mods and load order.')).toBeInTheDocument();
    });
  });

  Scenario('Settings has a section navigation', ({ Given, When, Then, And }) => {
    Given('the launcher open on the Settings screen', async () => {
      launch();
      await user.click(nav().getByRole('button', { name: 'Settings' }));
    });
    Then('I see a "Settings sections" navigation with "Game" and "Preferences"', () => {
      const sections = within(screen.getByRole('navigation', { name: 'Settings sections' }));
      expect(sections.getAllByRole('link').map((link) => link.textContent)).toEqual(['Game', 'Preferences']);
    });
    And('the "Game" section is marked as the current location', () => {
      const sections = within(screen.getByRole('navigation', { name: 'Settings sections' }));
      expect(sections.getByRole('link', { name: 'Game' })).toHaveAttribute('aria-current', 'location');
      expect(sections.getByRole('link', { name: 'Preferences' })).not.toHaveAttribute('aria-current');
    });
    When('I select the "Preferences" section', async () => {
      const sections = within(screen.getByRole('navigation', { name: 'Settings sections' }));
      await user.click(sections.getByRole('link', { name: 'Preferences' }));
    });
    Then('the "Preferences" section is marked as the current location', () => {
      const sections = within(screen.getByRole('navigation', { name: 'Settings sections' }));
      expect(sections.getByRole('link', { name: 'Preferences' })).toHaveAttribute('aria-current', 'location');
      expect(sections.getByRole('link', { name: 'Game' })).not.toHaveAttribute('aria-current');
    });
    And('the language setting is a row with its help text', () => {
      const row = screen.getByLabelText('Language').closest('.setting-row') as HTMLElement;
      expect(row).not.toBeNull();
      expect(within(row).getByText('Applies immediately and is remembered.')).toBeInTheDocument();
    });
  });

  Scenario('Session status uses a variant per state', ({ Given, Then }) => {
    Given('a running session', () => {
      launch({ session: { kind: 'running' } });
    });
    Then('the session status announces it is running with the running variant', () => {
      const status = screen.getByRole('status');
      expect(status).toHaveTextContent('Running');
      expect(status).toHaveClass('session-status', 'session-status-running');
    });
  });

  Scenario('Notices use banner variants', ({ Given, When, Then }) => {
    Given('an error from the last command', () => {
      launch({ error: { code: 'write_failed', message: 'disk' } });
    });
    Then('I see an alert banner with the error variant', () => {
      expect(screen.getByRole('alert')).toHaveClass('notice', 'notice-error');
    });
    When('the error is dismissed and a success notice arrives', () => {
      act(() => controller.setState({ error: null, notice: { kind: 'mod-removed' } }));
    });
    Then('I see a status banner with the success variant', () => {
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      expect(screen.getByRole('status')).toHaveClass('notice', 'notice-success');
    });
  });

  Scenario('The design tokens meet WCAG AA contrast', ({ Given, Then, And }) => {
    Given('the design tokens declared in styles.css', () => {
      expect(rootTokens(css).size).toBeGreaterThan(10);
    });
    Then('body text on the background and on every surface level has a contrast ratio of at least 4.5', () => {
      for (const surface of ['--bg', '--surface-1', '--surface-2', '--surface-3']) {
        expect(contrast(token('--text'), token(surface)), surface).toBeGreaterThanOrEqual(4.5);
      }
    });
    And('muted text on the surfaces has a contrast ratio of at least 4.5', () => {
      for (const surface of ['--bg', '--surface-1', '--surface-2', '--surface-3']) {
        expect(contrast(token('--text-muted'), token(surface)), surface).toBeGreaterThanOrEqual(4.5);
      }
    });
    And('the Play label on the accent has a contrast ratio of at least 4.5', () => {
      expect(contrast(token('--on-accent'), token('--accent'))).toBeGreaterThanOrEqual(4.5);
      expect(contrast(token('--on-accent'), token('--accent-hover'))).toBeGreaterThanOrEqual(4.5);
    });
    And('the accent text on the surfaces has a contrast ratio of at least 4.5', () => {
      for (const surface of ['--bg', '--surface-1', '--surface-2', '--surface-3']) {
        expect(contrast(token('--accent-text'), token(surface)), surface).toBeGreaterThanOrEqual(4.5);
      }
    });
    And('the focus ring on the background has a contrast ratio of at least 3', () => {
      expect(contrast(token('--focus-ring'), token('--bg'))).toBeGreaterThanOrEqual(3);
      expect(contrast(token('--focus-ring'), token('--surface-1'))).toBeGreaterThanOrEqual(3);
    });
  });

  Scenario('Focus is always visible', ({ Given, Then, And }) => {
    Given('the stylesheet', () => {
      expect(css.length).toBeGreaterThan(0);
    });
    Then('every :focus-visible rule draws an outline using the focus token', () => {
      const rules = [...css.matchAll(/([^{}]+)\{([^}]*)\}/g)].filter(([, selector = '']) =>
        selector.includes(':focus-visible'),
      );
      expect(rules.length).toBeGreaterThan(0);
      for (const [, selector = '', body = ''] of rules) {
        expect(body, selector.trim()).toMatch(/outline:[^;]*var\(--focus-ring\)/);
      }
    });
    And('no rule removes the outline without a replacement', () => {
      // Catches "none", "0", "0px" and the longhands, with or without a trailing semicolon.
      const removes = /outline(-style|-width)?\s*:\s*(none|0[a-z%]*)\s*(!important\s*)?(;|$)/m;
      const rules = [...css.matchAll(/([^{}]+)\{([^}]*)\}/g)].filter(([, , body = '']) => removes.test(body));
      for (const [, selector = '', body = ''] of rules) {
        // A rule may drop the native outline only if it paints a replacement.
        expect(body, selector.trim()).toMatch(/box-shadow|border-color/);
      }
    });
  });

  Scenario('Motion respects the reduced-motion preference', ({ Given, Then }) => {
    Given('the stylesheet', () => {
      expect(css.length).toBeGreaterThan(0);
    });
    Then('it declares a prefers-reduced-motion rule that disables animations and transitions', () => {
      const block = /@media\s*\(prefers-reduced-motion:\s*reduce\)\s*\{([\s\S]*?\})\s*\}/.exec(css)?.[1] ?? '';
      expect(block).toMatch(/animation:\s*none/);
      expect(block).toMatch(/transition:\s*none/);
    });
  });

  Scenario('The stylesheet stays restrained', ({ Given, Then, And }) => {
    Given('the stylesheet', () => {
      expect(css.length).toBeGreaterThan(0);
    });
    Then('no rule uses clip-path', () => {
      expect(css).not.toMatch(/clip-path/);
    });
    And('the radius tokens are between 4px and 8px', () => {
      const radii = [...rootTokens(css)].filter(([name]) => name.startsWith('--radius'));
      expect(radii.length).toBeGreaterThan(0);
      for (const [name, value] of radii) {
        const px = Number(/^(\d+)px$/.exec(value)?.[1]);
        expect(px, name).toBeGreaterThanOrEqual(4);
        expect(px, name).toBeLessThanOrEqual(8);
      }
    });
  });
});
