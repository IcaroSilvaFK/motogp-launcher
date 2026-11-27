// Binds tests/features/frontend/welcome.feature: the real App over a fake controller, a real catalog
// controller on a scripted gateway and an in-memory WelcomeStore (the port main.tsx wires).
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import { createCatalogController } from '../../application';
import type { WelcomeStore } from '../../application';
import type { Catalog, GameInfo } from '../../domain';
import { App } from '../App';
import { createFakeController } from './fakeController';
import type { FakeController } from './fakeController';
import { loadFrontendFeature } from './features';

type MemoryWelcomeStore = WelcomeStore & { value: boolean | null; saves: boolean[] };

function memoryWelcomeStore(initial: boolean | null): MemoryWelcomeStore {
  const store: MemoryWelcomeStore = {
    value: initial,
    saves: [],
    load: () => store.value,
    save: (seen) => {
      store.value = seen;
      store.saves.push(seen);
    },
  };
  return store;
}

const brokenWelcomeStore: WelcomeStore = {
  load: () => {
    throw new Error('storage unavailable');
  },
  save: () => {
    throw new Error('storage unavailable');
  },
};

const motogp: GameInfo = {
  name: 'MotoGP 25',
  root: 'C:/Games/MotoGP25',
  mods_dir: 'C:/Games/MotoGP25/MotoGP25/Content/Paks/~mods',
  app_id: 2_581_700,
};

const emptyCatalog: Catalog = { entries: [], fetchedAt: 0, stale: false, source: 'remote', skipped: 0 };

let store: WelcomeStore;
let controller: FakeController;
let user: ReturnType<typeof userEvent.setup>;

const dialog = () => screen.getByRole('dialog', { name: /./ });
const queryDialog = () => screen.queryByRole('dialog');

function open() {
  user = userEvent.setup();
  render(
    <App
      controller={controller}
      locale="en"
      welcomeStore={store}
      catalog={createCatalogController({
        gateway: {
          getCatalog: () => Promise.resolve(emptyCatalog),
          refreshCatalog: () => Promise.resolve(emptyCatalog),
          openCatalogPage: () => Promise.resolve(),
        },
      })}
    />,
  );
}

function expectStep(step: number) {
  expect(dialog()).toHaveAttribute('aria-modal', 'true');
  expect(within(dialog()).getByText(`Step ${step} of 3`)).toBeInTheDocument();
}

function expectSeen() {
  const memory = store as MemoryWelcomeStore;
  expect(memory.value).toBe(true);
  expect(memory.saves).toEqual([true]);
}

function neverOpened() {
  store = memoryWelcomeStore(null);
  controller = createFakeController();
}

const feature = await loadFrontendFeature('welcome');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  Scenario('First launch shows the welcome', ({ Given, When, Then, And }) => {
    Given('the launcher has never been opened', neverOpened);
    When('the launcher opens', open);
    Then('the welcome flow is shown at step 1 of 3', () => {
      expectStep(1);
      expect(dialog()).toHaveAccessibleName('Welcome to MotoGP Launcher');
    });
    And('the welcome title has the focus', () => {
      expect(within(dialog()).getByRole('heading', { level: 2 })).toHaveFocus();
    });
  });

  Scenario('The game is detected', ({ Given, When, Then, And }) => {
    Given('the launcher has never been opened', neverOpened);
    And('the game folder can be detected', () => {
      controller.detectGames = vi.fn(() => {
        controller.setState({ detected: [motogp] });
        return Promise.resolve();
      });
      controller.useGame = vi.fn(() => {
        controller.setState({ game: motogp });
        return Promise.resolve();
      });
    });
    When('the launcher opens', open);
    Then('step 1 shows the detected path and a "Looks right" action', async () => {
      expectStep(1);
      expect(await within(dialog()).findByText(motogp.root)).toBeInTheDocument();
      expect(within(dialog()).getByRole('button', { name: 'Looks right' })).toBeEnabled();
      expect(controller.detectGames).toHaveBeenCalledTimes(1);
    });
    When('I confirm the detected folder', async () => {
      await user.click(within(dialog()).getByRole('button', { name: 'Looks right' }));
    });
    Then('the detected folder is configured', () => {
      expect(controller.useGame).toHaveBeenCalledWith(motogp.root);
    });
    And('step 2 explains how mods work', async () => {
      await waitFor(() => expectStep(2));
      const steps = within(dialog()).getByRole('list');
      expect(within(steps).getAllByRole('listitem')).toHaveLength(3);
      expect(within(dialog()).getByRole('heading', { level: 2, name: 'How mods work' })).toBeInTheDocument();
    });
  });

  Scenario('The game is not detected', ({ Given, When, Then, And }) => {
    Given('the launcher has never been opened', neverOpened);
    And('no game folder can be detected', () => {
      controller.detectGames = vi.fn(() => {
        controller.setState({ detected: [] });
        return Promise.resolve();
      });
      controller.chooseGameDirectory = vi.fn(() => {
        controller.setState({ error: { code: 'not_unreal_game', message: 'not a game' } });
        return Promise.resolve();
      });
    });
    When('the launcher opens', open);
    Then('step 1 asks me to pick the game folder', async () => {
      expect(await within(dialog()).findByRole('button', { name: 'Choose game folder' })).toBeEnabled();
      expect(within(dialog()).queryByRole('button', { name: 'Looks right' })).toBeNull();
    });
    When('I pick a folder that is not the game', async () => {
      await user.click(within(dialog()).getByRole('button', { name: 'Choose game folder' }));
    });
    Then('step 1 shows why the folder was rejected', () => {
      expect(controller.chooseGameDirectory).toHaveBeenCalledTimes(1);
      expect(within(dialog()).getByRole('alert')).toHaveTextContent('This folder is not an Unreal Engine game.');
      expectStep(1);
    });
  });

  Scenario('Finishing goes to Discover and does not show the welcome again', ({ Given, When, Then, And }) => {
    Given('the launcher has never been opened', neverOpened);
    And('a game is configured', () => {
      controller.setState({ game: motogp });
    });
    When('the launcher opens', open);
    And('I go through the three steps and choose "Go to Discover"', async () => {
      expect(within(dialog()).getByText(motogp.root)).toBeInTheDocument();
      await user.click(within(dialog()).getByRole('button', { name: 'Looks right' }));
      expectStep(2);
      await user.click(within(dialog()).getByRole('button', { name: 'Next' }));
      expectStep(3);
      await user.click(within(dialog()).getByRole('button', { name: 'Go to Discover' }));
    });
    Then('the welcome is closed and Discover is shown', () => {
      expect(queryDialog()).toBeNull();
      expect(screen.getByRole('heading', { level: 2, name: 'Discover' })).toBeInTheDocument();
      // A configured game is never reconfigured by "Looks right".
      expect(controller.useGame).not.toHaveBeenCalled();
    });
    And('the welcome is remembered as seen', expectSeen);
    When('the launcher opens again', () => {
      cleanup();
      open();
    });
    Then('the welcome flow is not shown', () => {
      expect(queryDialog()).toBeNull();
    });
  });

  Scenario('Skipping does not show the welcome again and Settings offers it', ({ Given, When, Then, And }) => {
    Given('the launcher has never been opened', neverOpened);
    When('the launcher opens', open);
    And('I skip the welcome', async () => {
      await user.click(within(dialog()).getByRole('button', { name: 'Skip' }));
      expect(queryDialog()).toBeNull();
    });
    Then('the welcome is remembered as seen', expectSeen);
    When('I open Settings and choose "Show welcome again"', async () => {
      await user.click(screen.getByRole('button', { name: 'Settings' }));
      await user.click(screen.getByRole('button', { name: 'Show welcome again' }));
    });
    Then('the welcome flow is shown at step 1 of 3', () => {
      expectStep(1);
    });
  });

  Scenario('Escape skips the welcome', ({ Given, When, Then, And }) => {
    Given('the launcher has never been opened', neverOpened);
    When('the launcher opens', open);
    And('I press Escape', async () => {
      await user.keyboard('{Escape}');
    });
    Then('the welcome is closed', () => {
      expect(queryDialog()).toBeNull();
    });
    And('the welcome is remembered as seen', expectSeen);
  });

  Scenario('A broken store does not block the launcher', ({ Given, When, Then }) => {
    Given('a welcome store that fails to read and to save', () => {
      store = brokenWelcomeStore;
      controller = createFakeController();
    });
    When('the launcher opens', open);
    Then('the welcome flow is not shown', () => {
      expect(queryDialog()).toBeNull();
      expect(screen.getByRole('heading', { level: 2, name: 'Library' })).toBeInTheDocument();
    });
  });
});
