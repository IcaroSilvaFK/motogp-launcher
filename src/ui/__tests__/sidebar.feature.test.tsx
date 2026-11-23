// Binds tests/features/frontend/sidebar.feature: the real App over a fake controller and a fake
// NavPreferenceStore, so the persistence path is the one main.tsx wires.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect } from 'vitest';
import type { NavPreferenceStore } from '../../application';
import { App } from '../App';
import { createFakeController } from './fakeController';
import { loadFrontendFeature } from './features';

type MemoryNavStore = NavPreferenceStore & { value: boolean | null; saves: boolean[] };

function memoryNavStore(initial: boolean | null): MemoryNavStore {
  const store: MemoryNavStore = {
    value: initial,
    saves: [],
    load: () => store.value,
    save: (collapsed) => {
      store.value = collapsed;
      store.saves.push(collapsed);
    },
  };
  return store;
}

const brokenNavStore: NavPreferenceStore = {
  load: () => {
    throw new Error('storage unavailable');
  },
  save: () => {
    throw new Error('storage unavailable');
  },
};

let store: NavPreferenceStore;
let user: ReturnType<typeof userEvent.setup>;

const sidebar = () => screen.getByRole('complementary');
const toggle = () => within(sidebar()).getByRole('button', { name: /sidebar$/ });

function expectCollapsed(collapsed: boolean) {
  expect(sidebar()).toHaveAttribute('data-collapsed', String(collapsed));
  expect(toggle()).toHaveAttribute('aria-expanded', String(!collapsed));
}

function open() {
  user = userEvent.setup();
  render(<App controller={createFakeController()} locale="en" navStore={store} />);
}

const feature = await loadFrontendFeature('sidebar');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  Scenario('The collapsed choice survives a restart', ({ Given, When, Then }) => {
    Given('a stored preference with the sidebar collapsed', () => {
      store = memoryNavStore(true);
    });
    When('the launcher opens', () => {
      open();
    });
    Then('the sidebar is collapsed', () => {
      expectCollapsed(true);
      expect(toggle()).toHaveAccessibleName('Expand sidebar');
    });
    When('I expand the sidebar', async () => {
      await user.click(within(sidebar()).getByRole('button', { name: 'Expand sidebar' }));
    });
    Then('the stored preference says the sidebar is expanded', () => {
      expectCollapsed(false);
      const memory = store as MemoryNavStore;
      expect(memory.value).toBe(false);
      expect(memory.saves).toEqual([false]);
    });
  });

  Scenario('A broken preference store does not break the launcher', ({ Given, When, Then, And }) => {
    Given('a preference store that fails to read and to save', () => {
      store = brokenNavStore;
    });
    When('the launcher opens', () => {
      open();
    });
    Then('the sidebar is expanded', () => {
      expectCollapsed(false);
      expect(screen.getByRole('navigation', { name: 'Main navigation' })).toBeInTheDocument();
    });
    And('toggling the sidebar still collapses it', async () => {
      await user.click(within(sidebar()).getByRole('button', { name: 'Collapse sidebar' }));
      expectCollapsed(true);
    });
  });
});
