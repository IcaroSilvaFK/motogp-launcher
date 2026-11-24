// Binds tests/features/frontend/status-bar-artwork.feature: the real App over a fake controller
// whose launcher state carries (or not) the Steam artwork `data:` URL.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { expect } from 'vitest';
import type { LauncherState } from '../../application';
import { App } from '../App';
import { createFakeController } from './fakeController';
import type { FakeController } from './fakeController';
import { loadFrontendFeature } from './features';

const ARTWORK = 'data:image/png;base64,AQID';
const OTHER_ARTWORK = 'data:image/jpeg;base64,BAUG';
const steamGame = {
  name: 'MotoGP 26',
  root: 'D:/Steam/MotoGP26',
  mods_dir: 'D:/Steam/MotoGP26/~mods',
  app_id: 3875050,
};

let controller: FakeController;

function open(patch: Partial<LauncherState>) {
  controller = createFakeController(patch);
  render(<App controller={controller} locale="en" />);
}

function tile(): HTMLElement {
  const footer = screen.getByRole('contentinfo');
  const found = footer.querySelector<HTMLElement>('.game-tile');
  if (found === null) throw new Error('game tile not rendered');
  return found;
}

function expectGenericIcon() {
  const gameTile = tile();
  expect(gameTile).toHaveAttribute('aria-hidden', 'true');
  expect(gameTile.querySelector('img')).toBeNull();
  expect(gameTile.querySelector('svg.icon')).not.toBeNull();
  expect(gameTile).not.toHaveClass('has-artwork');
}

const feature = await loadFrontendFeature('status-bar-artwork');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  Scenario('Show the Steam artwork of the configured game', ({ Given, When, Then }) => {
    let patch: Partial<LauncherState> = {};
    Given('a configured Steam game with cached artwork', () => {
      patch = { game: steamGame, artwork: ARTWORK };
    });
    When('the launcher opens', () => {
      open(patch);
    });
    Then('the status bar shows the artwork as a decorative image', () => {
      const gameTile = tile();
      expect(gameTile).toHaveAttribute('aria-hidden', 'true');
      expect(gameTile).toHaveClass('has-artwork');
      const image = gameTile.querySelector('img');
      expect(image).not.toBeNull();
      expect(image).toHaveAttribute('src', ARTWORK);
      expect(image).toHaveAttribute('alt', '');
      expect(gameTile.querySelector('svg')).toBeNull();
      // Decorative: the image is not exposed to assistive technology.
      expect(screen.queryByRole('img', { name: /./ })).toBeNull();
    });
  });

  Scenario('Fall back to the generic icon when there is no artwork', ({ Given, When, Then }) => {
    let patch: Partial<LauncherState> = {};
    Given('a configured game without cached artwork', () => {
      patch = { game: steamGame, artwork: null };
    });
    When('the launcher opens', () => {
      open(patch);
    });
    Then('the status bar shows the generic game icon', () => {
      expectGenericIcon();
    });
  });

  Scenario('Fall back when the image cannot be displayed', ({ Given, Then }) => {
    Given('the artwork fails to load', () => {
      open({ game: steamGame, artwork: ARTWORK });
      const image = tile().querySelector('img');
      if (image === null) throw new Error('artwork not rendered');
      fireEvent.error(image);
    });
    Then('the status bar shows the generic game icon', () => {
      expectGenericIcon();
      // A new artwork URL gets a fresh chance to load.
      act(() => controller.setState({ artwork: OTHER_ARTWORK }));
      expect(tile().querySelector('img')).toHaveAttribute('src', OTHER_ARTWORK);
    });
  });

  Scenario('No game configured', ({ Given, Then }) => {
    Given('no game is configured', () => {
      // A stale artwork left in the state must not be shown without a game.
      open({ game: null, artwork: ARTWORK });
    });
    Then('the status bar shows the generic game icon', () => {
      expectGenericIcon();
    });
  });
});
