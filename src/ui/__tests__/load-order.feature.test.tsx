// Binds tests/features/frontend/load-order.feature end to end: the real application controller
// and the real UI, with only the ports replaced by the in-memory fakes. The fake gateway computes
// conflicts like the back end does: same file name (case-insensitive), the last mod in the order wins and names the path.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect } from 'vitest';
import { createLauncherController } from '../../application';
import type { FakeGatewayOptions } from '../../application/__tests__/fakes';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { FileConflict, ModEntry } from '../../domain';
import { App } from '../App';
import { loadFrontendFeature } from './features';

const alpha: ModEntry = { id: 'alpha', name: 'Alpha', files: ['Shared_P.pak', 'Alpha_P.pak'], size_bytes: 2048 };
const beta: ModEntry = { id: 'beta', name: 'Beta', files: ['Shared_P.pak'], size_bytes: 1024 };
const gamma: ModEntry = { id: 'gamma', name: 'Gamma', files: ['Gamma_P.pak'], size_bytes: 512 };
const lonelyBeta: ModEntry = { ...beta, files: ['Beta_P.pak'] };

/** Mirrors the domain rule: grouped by lowercased file name; the last mod wins and its spelling names the path. */
function detectConflicts(mods: ModEntry[], modIds: string[]): FileConflict[] {
  const byFile = new Map<string, { path: string; mod_ids: string[] }>();
  for (const id of modIds) {
    for (const file of mods.find((mod) => mod.id === id)?.files ?? []) {
      const key = file.toLowerCase();
      const entry = byFile.get(key) ?? { path: file, mod_ids: [] };
      if (!entry.mod_ids.includes(id)) entry.mod_ids.push(id);
      entry.path = file;
      byFile.set(key, entry);
    }
  }
  return [...byFile.values()]
    .filter((entry) => entry.mod_ids.length > 1)
    .map((entry) => ({ ...entry, winner: entry.mod_ids.at(-1) ?? '' }));
}

function libraryOf(mods: ModEntry[], modIds: string[]): FakeGatewayOptions {
  return {
    mods,
    profiles: [
      { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
      { id: 'mine', name: 'Mine', mod_ids: modIds },
    ],
    config: { game_dir: null, active_profile: 'mine' },
    conflictsFor: (ids) => detectConflicts(mods, ids),
  };
}

async function launch(options: FakeGatewayOptions) {
  const gateway = new FakeGateway(options);
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
  });
  const user = userEvent.setup();
  render(<App controller={controller} locale="en" />);
  await waitFor(() => expect(controller.getState().status).toBe('ready'));
  await waitFor(() => expect(controller.getState().busy).toBe(false));
  return { gateway, controller, user };
}

type World = Awaited<ReturnType<typeof launch>>;

const openScreen = (user: World['user'], name: string) =>
  user.click(within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', { name }));

async function openEditor(world: World) {
  await openScreen(world.user, 'Profiles');
  await world.user.click(screen.getByRole('button', { name: 'Mine' }));
}

const loadOrder = () => screen.getByRole('list', { name: 'Load order' });
const orderNames = () =>
  within(loadOrder())
    .getAllByRole('listitem')
    .map((item) => item.querySelector('.order-name')?.textContent);
const conflictsPanel = () => screen.getByRole('region', { name: 'Conflicts' });
/** Text of each involved mod of a conflicting file, in load order; the winner reads "<name> Wins". */
const modsOf = (path: string) =>
  within(within(conflictsPanel()).getByRole('list', { name: path }))
    .getAllByRole('listitem')
    .map((item) => item.textContent);
const rowOf = (modName: string) =>
  within(screen.getByRole('list', { name: 'Library' }))
    .getByText(modName, { selector: '.mod-name' })
    .closest('li') as HTMLElement;
const savedMods = (gateway: FakeGateway) => gateway.callsOf('setProfileMods').map((call) => call.args);

const feature = await loadFrontendFeature('load-order');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  let world: World | undefined;
  const getWorld = () => {
    if (world === undefined) throw new Error('the app was not launched');
    return world;
  };

  Scenario('Reorder the load order with the keyboard', ({ Given, When, Then }) => {
    Given('a profile with mods "Alpha" and "Beta" in that order', async () => {
      world = await launch(libraryOf([alpha, beta], ['alpha', 'beta']));
      await openEditor(world);
      expect(orderNames()).toEqual(['Alpha', 'Beta']);
      expect(screen.getByText('Mods lower in the list load later and win conflicts.')).toBeInTheDocument();
    });
    When('I move "Beta" up with the keyboard and save', async () => {
      const { user } = getWorld();
      screen.getByRole('button', { name: 'Move Beta up' }).focus();
      await user.keyboard('{Enter}');
      expect(orderNames()).toEqual(['Beta', 'Alpha']);
      // "Move Beta up" is now disabled, so focus stays on the moved item through its other button.
      expect(screen.getByRole('button', { name: 'Move Beta up' })).toBeDisabled();
      expect(screen.getByRole('button', { name: 'Move Beta down' })).toHaveFocus();
      await user.keyboard('{Enter}');
      expect(orderNames()).toEqual(['Alpha', 'Beta']);
      expect(screen.getByRole('button', { name: 'Move Beta up' })).toHaveFocus();
      await user.keyboard('{Enter}');
      screen.getByRole('button', { name: 'Save' }).focus();
      await user.keyboard('{Enter}');
    });
    Then('the profile is saved with the order "Beta", "Alpha"', async () => {
      const { gateway } = getWorld();
      await waitFor(() => expect(savedMods(gateway)).toEqual([['mine', ['beta', 'alpha']]]));
    });
  });

  Scenario('The conflicts panel previews the draft order', ({ Given, When, Then }) => {
    Given('"Alpha" and "Beta" ship the same file', async () => {
      world = await launch(libraryOf([alpha, beta], ['alpha', 'beta']));
    });
    When('both are in the draft order', async () => {
      await openEditor(getWorld());
      expect(orderNames()).toEqual(['Alpha', 'Beta']);
    });
    Then('the conflicts panel lists the file, both mods and "Beta" as the winner', async () => {
      await waitFor(() => expect(within(conflictsPanel()).getByText('Shared_P.pak')).toBeInTheDocument());
      expect(modsOf('Shared_P.pak')).toEqual(['Alpha', 'Beta Wins']);
      expect(within(conflictsPanel()).getByRole('status')).toHaveTextContent('1 conflicting file');
    });
    When('I move "Beta" above "Alpha"', async () => {
      const { user } = getWorld();
      await user.click(screen.getByRole('button', { name: 'Move Beta up' }));
    });
    Then('the panel shows "Alpha" as the winner', async () => {
      const { gateway } = getWorld();
      await waitFor(() => expect(modsOf('Shared_P.pak')).toEqual(['Beta', 'Alpha Wins']));
      // The preview follows the unsaved draft; nothing is saved.
      expect(gateway.callsOf('listConflicts').at(-1)?.args).toEqual([['beta', 'alpha']]);
      expect(savedMods(gateway)).toEqual([]);
    });
  });

  Scenario('No conflicts', ({ Given, Then }) => {
    Given('the draft mods share no file', async () => {
      world = await launch(libraryOf([alpha, lonelyBeta], ['alpha', 'beta']));
      await openEditor(world);
    });
    Then('the conflicts panel says there are no conflicts', async () => {
      const { gateway } = getWorld();
      await waitFor(() => expect(gateway.callsOf('listConflicts').at(-1)?.args).toEqual([['alpha', 'beta']]));
      await waitFor(() => expect(within(conflictsPanel()).getByRole('status')).toHaveTextContent('No conflicts'));
      expect(within(conflictsPanel()).queryByRole('list')).not.toBeInTheDocument();
      // Fewer than two mods cannot conflict: the panel stays empty without asking the back end.
      const calls = gateway.callsOf('listConflicts').length;
      await getWorld().user.click(screen.getByRole('checkbox', { name: 'Beta' }));
      expect(orderNames()).toEqual(['Alpha']);
      expect(within(conflictsPanel()).getByRole('status')).toHaveTextContent('No conflicts');
      expect(gateway.callsOf('listConflicts')).toHaveLength(calls);
    });
  });

  Scenario('Library rows show a conflict badge for the active profile', ({ Given, When, Then, And }) => {
    Given('the active profile contains conflicting mods "Alpha" then "Beta"', async () => {
      world = await launch(libraryOf([alpha, beta, gamma], ['alpha', 'beta', 'gamma']));
    });
    When('I open the library', async () => {
      await openScreen(getWorld().user, 'Library');
      expect(screen.getByRole('heading', { name: 'Library' })).toBeInTheDocument();
    });
    Then('the "Alpha" row shows an "overridden" conflict badge', () => {
      const row = rowOf('Alpha');
      const badge = within(row).getByText('Overridden').closest('.badge');
      expect(badge).toHaveAttribute('title', '1 file overridden by Beta');
      expect(within(row).getByText('1 file overridden by Beta')).toBeInTheDocument();
    });
    And('the "Beta" row shows a "wins" conflict badge', () => {
      const row = rowOf('Beta');
      const badge = within(row).getByText('Wins').closest('.badge');
      expect(badge).toHaveAttribute('title', 'Wins 1 file over Alpha');
      expect(within(row).queryByText('Overridden')).not.toBeInTheDocument();
    });
    And('mods without conflicts show no conflict badge', () => {
      const row = rowOf('Gamma');
      expect(within(row).queryByText('Wins')).not.toBeInTheDocument();
      expect(within(row).queryByText('Overridden')).not.toBeInTheDocument();
    });
  });
});
