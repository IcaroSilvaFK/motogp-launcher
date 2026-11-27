// Binds tests/features/frontend/library-sort-filter.feature end to end: the real application
// controller and the real UI, with only the ports replaced by the in-memory fakes.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect } from 'vitest';
import type { LibrarySort, LibrarySortStore } from '../../application';
import { createLauncherController } from '../../application';
import type { FakeGatewayOptions } from '../../application/__tests__/fakes';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { ModEntry } from '../../domain';
import { App } from '../App';
import { loadFrontendFeature } from './features';

const mod = (name: string, size_bytes: number, files: string[] = [`${name}.pak`]): ModEntry => ({
  id: name.toLowerCase(),
  name,
  files,
  size_bytes,
});

function memorySortStore(initial: LibrarySort | null = null): LibrarySortStore & { value: LibrarySort | null } {
  const store = {
    value: initial,
    load: () => store.value,
    save: (sort: LibrarySort) => {
      store.value = sort;
    },
  };
  return store;
}

async function launch(options: FakeGatewayOptions, librarySortStore?: LibrarySortStore) {
  const gateway = new FakeGateway(options);
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
  });
  const user = userEvent.setup();
  render(<App controller={controller} locale="en" librarySortStore={librarySortStore} />);
  await waitFor(() => expect(controller.getState().status).toBe('ready'));
  await waitFor(() => expect(controller.getState().busy).toBe(false));
  return { user };
}

type World = Awaited<ReturnType<typeof launch>>;

const listed = () =>
  within(screen.getByRole('list', { name: 'Library' }))
    .getAllByRole('listitem')
    .map((item) => item.querySelector('.mod-name')?.textContent);
const sortBy = (user: World['user'], label: 'Name' | 'Size') =>
  user.selectOptions(screen.getByRole('combobox', { name: 'Sort by' }), label);

const three: FakeGatewayOptions = { mods: [mod('Zeta', 10), mod('Alpha', 30), mod('Mid', 20)] };

const feature = await loadFrontendFeature('library-sort-filter');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  let world: World | undefined;
  const getWorld = () => {
    if (world === undefined) throw new Error('the app was not launched');
    return world;
  };

  Scenario('Sorting by name', ({ Given, When, Then }) => {
    Given('the mods "Zeta", "Alpha" and "Mid" are installed', async () => {
      world = await launch(three);
    });
    When('the user sorts by name', async () => {
      await sortBy(getWorld().user, 'Name');
    });
    Then('the list reads "Alpha", "Mid", "Zeta"', () => {
      expect(listed()).toEqual(['Alpha', 'Mid', 'Zeta']);
    });
  });

  Scenario('Sorting by size', ({ Given, When, Then }) => {
    Given('the mods "Zeta", "Alpha" and "Mid" are installed', async () => {
      world = await launch(three);
    });
    When('the user sorts by size', async () => {
      await sortBy(getWorld().user, 'Size');
    });
    Then('the list reads "Alpha", "Mid", "Zeta" from the largest to the smallest', () => {
      expect(listed()).toEqual(['Alpha', 'Mid', 'Zeta']);
    });
  });

  Scenario('The sort choice is remembered', ({ Given, When, Then }) => {
    const store = memorySortStore();
    Given('the user sorted by size', async () => {
      world = await launch({ mods: [mod('Alpha', 1), mod('Beta', 99)] }, store);
      await sortBy(getWorld().user, 'Size');
      expect(store.value).toBe('size');
    });
    When('the launcher is reopened', async () => {
      cleanup();
      world = await launch({ mods: [mod('Alpha', 1), mod('Beta', 99)] }, store);
    });
    Then('the Library is still sorted by size', () => {
      expect(screen.getByRole('combobox', { name: 'Sort by' })).toHaveValue('size');
      expect(listed()).toEqual(['Beta', 'Alpha']);
    });
  });

  Scenario('Showing only conflicting mods', ({ Given, When, Then }) => {
    Given('"A" and "B" conflict and "C" does not', async () => {
      const mods = [mod('A', 1, ['Shared_P.pak']), mod('B', 1, ['Shared_P.pak']), mod('C', 1, ['C_P.pak'])];
      world = await launch({
        mods,
        profiles: [
          { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
          { id: 'mine', name: 'Mine', mod_ids: ['a', 'b', 'c'] },
        ],
        config: { game_dir: null, active_profile: 'mine' },
        conflictsFor: () => [{ path: 'Shared_P.pak', mod_ids: ['a', 'b'], winner: 'b' }],
      });
      expect(listed()).toEqual(['A', 'B', 'C']);
    });
    When('the user turns on "Has conflicts"', async () => {
      await getWorld().user.click(screen.getByRole('button', { name: /^Has conflicts/ }));
    });
    Then('only "A" and "B" are listed', () => {
      expect(listed()).toEqual(['A', 'B']);
    });
  });
});
