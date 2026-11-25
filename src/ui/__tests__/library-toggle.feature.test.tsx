// Binds tests/features/frontend/library-toggle.feature end to end: the real application
// controller and the real UI, with only the ports replaced by the in-memory fakes.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect } from 'vitest';
import { createLauncherController } from '../../application';
import type { FakeGatewayOptions } from '../../application/__tests__/fakes';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { ModEntry } from '../../domain';
import { App } from '../App';
import { loadFrontendFeature } from './features';

const fast: ModEntry = { id: 'fast', name: 'Fast Bike', files: ['fast.pak'], size_bytes: 2048 };
const rain: ModEntry = { id: 'rain', name: 'Rain Tyres', files: ['rain.pak'], size_bytes: 1024 };

const LOCKED = 'Profile changes are locked while the game is running.';
const VANILLA = 'Vanilla always runs without mods. Pick another profile to add mods.';

async function launch(options: FakeGatewayOptions) {
  const gateway = new FakeGateway(options);
  const session = new FakeSessionEvents();
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session,
    drop: new FakeDropEvents(),
  });
  const user = userEvent.setup();
  render(<App controller={controller} locale="en" />);
  await waitFor(() => expect(controller.getState().status).toBe('ready'));
  await waitFor(() => expect(controller.getState().busy).toBe(false));
  return { gateway, session, controller, user };
}

type World = Awaited<ReturnType<typeof launch>>;

const toggle = (modName: string, profileName = 'Mine') =>
  screen.getByRole('switch', { name: `Include ${modName} in ${profileName}` });
const facetCount = (label: string) =>
  within(screen.getByRole('button', { name: new RegExp(`^${label}`) })).getByText(/^\d+$/);
const savedMods = (gateway: FakeGateway) => gateway.callsOf('setProfileMods').map((call) => call.args);

const feature = await loadFrontendFeature('library-toggle');

describeFeature(feature, ({ Background, Scenario, AfterEachScenario }) => {
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  // The app is launched lazily so a scenario's own Given can adjust the Background's data.
  let options: FakeGatewayOptions = {};
  let world: World | undefined;
  const getWorld = async () => (world ??= await launch(options));

  Background(({ Given, And }) => {
    Given('the library has the mods "Fast Bike" and "Rain Tyres"', () => {
      world = undefined;
      options = { mods: [fast, rain] };
    });
    And('the active profile "Mine" contains only "Fast Bike"', () => {
      options = {
        ...options,
        profiles: [
          { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
          { id: 'mine', name: 'Mine', mod_ids: ['fast'] },
        ],
        config: { game_dir: null, active_profile: 'mine' },
      };
    });
  });

  Scenario('Each library row shows a switch bound to the active profile', ({ When, Then, And }) => {
    When('the user opens the Library', async () => {
      await getWorld();
      expect(screen.getByRole('heading', { name: 'Library' })).toBeInTheDocument();
    });
    Then('the switch for "Fast Bike" is on', () => {
      expect(toggle('Fast Bike')).toHaveAttribute('aria-checked', 'true');
      const row = toggle('Fast Bike').closest('li') as HTMLElement;
      expect(within(row).getByText('In active profile')).toBeInTheDocument();
    });
    And('the switch for "Rain Tyres" is off', () => {
      expect(toggle('Rain Tyres')).toHaveAttribute('aria-checked', 'false');
      expect(toggle('Rain Tyres')).not.toHaveAttribute('aria-disabled');
      const row = toggle('Rain Tyres').closest('li') as HTMLElement;
      expect(within(row).getByText('Not in active profile')).toBeInTheDocument();
    });
  });

  Scenario('Turning a mod on adds it to the active profile', ({ When, Then, And }) => {
    When('the user turns on the switch for "Rain Tyres"', async () => {
      const { user } = await getWorld();
      await user.click(toggle('Rain Tyres'));
    });
    Then('the active profile is saved with "Fast Bike" and "Rain Tyres" in that order', async () => {
      const { gateway } = await getWorld();
      await waitFor(() => expect(savedMods(gateway)).toEqual([['mine', ['fast', 'rain']]]));
    });
    And('the switch for "Rain Tyres" is on', async () => {
      const { controller } = await getWorld();
      await waitFor(() => expect(toggle('Rain Tyres')).toHaveAttribute('aria-checked', 'true'));
      // The row itself is the feedback: no success notice per click.
      expect(controller.getState().notice).toBeNull();
      expect(screen.queryByText('Profile saved')).not.toBeInTheDocument();
    });
    And('the "In active profile" filter count is 2', () => {
      expect(facetCount('In active profile')).toHaveTextContent('2');
      expect(screen.getByText('2 mods · 2 in Mine')).toBeInTheDocument();
    });
  });

  Scenario('Turning a mod off removes it from the active profile', ({ When, Then, And }) => {
    When('the user turns off the switch for "Fast Bike"', async () => {
      const { user } = await getWorld();
      // Keyboard path: Space on the focused switch.
      toggle('Fast Bike').focus();
      await user.keyboard(' ');
    });
    Then('the active profile is saved with no mods', async () => {
      const { gateway } = await getWorld();
      await waitFor(() => expect(savedMods(gateway)).toEqual([['mine', []]]));
      await waitFor(() => expect(toggle('Fast Bike')).toHaveAttribute('aria-checked', 'false'));
    });
    And('the "Not in active profile" filter count is 2', () => {
      expect(facetCount('Not in active profile')).toHaveTextContent('2');
    });
  });

  Scenario('The current filter updates immediately', ({ Given, When, Then }) => {
    Given('the "In active profile" filter is selected', async () => {
      const { user } = await getWorld();
      await user.click(screen.getByRole('button', { name: /^In active profile/ }));
      expect(screen.queryByText('Rain Tyres')).not.toBeInTheDocument();
    });
    When('the user turns off the switch for "Fast Bike"', async () => {
      const { user } = await getWorld();
      await user.click(toggle('Fast Bike'));
    });
    Then('"Fast Bike" is no longer listed', async () => {
      await waitFor(() => expect(screen.queryByText('Fast Bike')).not.toBeInTheDocument());
    });
  });

  Scenario('Switches are locked while a game session is running', ({ Given, Then, And, When }) => {
    Given('a game session is running', async () => {
      const { session, controller } = await getWorld();
      session.emitSession({ state: 'started', cleaned: 0, started_at: null });
      await waitFor(() => expect(controller.getState().session.kind).toBe('running'));
    });
    Then('every switch is marked as disabled', () => {
      for (const element of screen.getAllByRole('switch')) {
        expect(element).toHaveAttribute('aria-disabled', 'true');
      }
    });
    And('each switch explains that profile changes are locked while the game is running', () => {
      for (const element of screen.getAllByRole('switch')) {
        expect(element).toHaveAttribute('title', LOCKED);
        expect(element).toHaveAccessibleDescription(LOCKED);
      }
    });
    When('the user clicks the switch for "Rain Tyres"', async () => {
      const { user } = await getWorld();
      await user.click(toggle('Rain Tyres'));
    });
    Then('the active profile is not changed', async () => {
      const { gateway } = await getWorld();
      expect(savedMods(gateway)).toEqual([]);
      expect(gateway.profiles.find((profile) => profile.id === 'mine')?.mod_ids).toEqual(['fast']);
      expect(toggle('Rain Tyres')).toHaveAttribute('aria-checked', 'false');
    });
  });

  Scenario('The Vanilla profile cannot receive mods', ({ Given, Then, And }) => {
    Given('the active profile is "Vanilla"', async () => {
      options = { ...options, config: { game_dir: null, active_profile: 'vanilla' } };
      await getWorld();
    });
    Then('every switch is off and marked as disabled', () => {
      const switches = screen.getAllByRole('switch');
      expect(switches).toHaveLength(2);
      for (const element of switches) {
        expect(element).toHaveAttribute('aria-checked', 'false');
        expect(element).toHaveAttribute('aria-disabled', 'true');
      }
    });
    And('each switch explains that Vanilla runs without mods', async () => {
      for (const element of screen.getAllByRole('switch')) {
        expect(element).toHaveAttribute('title', VANILLA);
        expect(element).toHaveAccessibleDescription(VANILLA);
      }
      const { user, gateway } = await getWorld();
      await user.click(toggle('Rain Tyres', 'Vanilla'));
      expect(savedMods(gateway)).toEqual([]);
    });
  });

  Scenario('A failed save shows the error', ({ Given, When, Then, And }) => {
    Given('saving the profile fails', async () => {
      const { gateway } = await getWorld();
      gateway.failNext('setProfileMods', { code: 'write_failed', message: 'disk full' });
    });
    When('the user turns on the switch for "Rain Tyres"', async () => {
      const { user } = await getWorld();
      await user.click(toggle('Rain Tyres'));
    });
    Then('an error notice is shown', async () => {
      expect(await screen.findByRole('alert')).toBeInTheDocument();
    });
    And('the switch for "Rain Tyres" is off', () => {
      expect(toggle('Rain Tyres')).toHaveAttribute('aria-checked', 'false');
    });
  });
});
