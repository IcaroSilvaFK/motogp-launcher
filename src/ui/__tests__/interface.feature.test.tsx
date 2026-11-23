// Binds tests/features/frontend/interface.feature end to end: the real application controller and
// the real UI, with only the ports replaced by the in-memory fakes.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { StrictMode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { createLauncherController } from '../../application';
import type { LauncherController } from '../../application';
import type { FakeGatewayOptions } from '../../application/__tests__/fakes';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { GameInfo, ModEntry } from '../../domain';
import { App } from '../App';
import { loadFrontendFeature } from './features';

const fakeRoot = 'C:/temp/MotoGPFake';
const steamRoot = 'D:/Steam/MotoGP25';
const steamGame: GameInfo = {
  name: 'MotoGP 25',
  root: steamRoot,
  mods_dir: `${steamRoot}/MotoGP25/Content/Paks/~mods`,
  app_id: 1234,
};
const noAppIdGame: GameInfo = {
  name: 'MotoGPFake',
  root: fakeRoot,
  mods_dir: `${fakeRoot}/MotoGPFake/Content/Paks/~mods`,
  app_id: null,
};
const modA: ModEntry = { id: 'A', name: 'Mod A', files: ['a.pak', 'a.utoc'], size_bytes: 2048 };
const modB: ModEntry = { id: 'B', name: 'Mod B', files: ['b.pak'], size_bytes: 500 };

interface World {
  log: string[];
  gateway: FakeGateway;
  picker: FakePicker;
  session: FakeSessionEvents;
  drop: FakeDropEvents;
  controller: LauncherController;
  user: ReturnType<typeof userEvent.setup>;
  unmount: () => void;
}

function createPorts(options: FakeGatewayOptions = {}) {
  const log: string[] = [];
  const gateway = new FakeGateway({ ...options, log });
  const picker = new FakePicker();
  const session = new FakeSessionEvents({ log });
  const drop = new FakeDropEvents({ log });
  return { log, gateway, picker, session, drop };
}

/** Mounts the real UI over a real controller and waits until the first load finished. */
async function launch(options: FakeGatewayOptions = {}, strict = false): Promise<World> {
  const ports = createPorts(options);
  const controller = createLauncherController({
    gateway: ports.gateway,
    picker: ports.picker,
    session: ports.session,
    drop: ports.drop,
  });
  const user = userEvent.setup();
  const tree = <App controller={controller} locale="en" />;
  const view = render(strict ? <StrictMode>{tree}</StrictMode> : tree);
  await waitFor(() => expect(controller.getState().status).toBe('ready'));
  await waitFor(() => expect(ports.session.activeListeners()).toBe(2));
  await waitFor(() => expect(ports.drop.activeListeners()).toBe(1));
  return { ...ports, controller, user, unmount: view.unmount };
}

const settled = (controller: LauncherController) =>
  waitFor(() => {
    expect(controller.getState().status).toBe('ready');
    expect(controller.getState().busy).toBe(false);
  });

const openScreen = (user: World['user'], name: string) =>
  user.click(within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', { name }));

const playButton = () => screen.getByRole('button', { name: 'Play' });

const feature = await loadFrontendFeature('interface');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  Scenario('Configure an Unreal folder without Steam', ({ Given, When, Then, And }) => {
    let world: World;

    Given('the fake folder "C:/temp/MotoGPFake/MotoGPFake/Content/Paks"', async () => {
      world = await launch();
    });

    When('I choose "C:/temp/MotoGPFake" in Settings', async () => {
      world.picker.queueDirectory(fakeRoot);
      await openScreen(world.user, 'Settings');
      await world.user.click(screen.getByRole('button', { name: 'Choose folder…' }));
    });

    Then('I see mods_dir ending in "Content/Paks/~mods"', async () => {
      const modsDir = await screen.findByText(/Content\/Paks\/~mods$/);
      expect(modsDir).toHaveTextContent('C:/temp/MotoGPFake/MotoGPFake/Content/Paks/~mods');
    });

    And('I see the App ID as not found', () => {
      expect(screen.getByText('not found')).toBeInTheDocument();
    });
  });

  Scenario('Use a detected game', ({ Given, When, Then, And }) => {
    let world: World;

    Given('detection returns a MotoGP with an App ID', async () => {
      world = await launch({ detected: [steamGame], games: { [steamRoot]: steamGame } });
    });

    When('I click "Use this"', async () => {
      await openScreen(world.user, 'Settings');
      await world.user.click(screen.getByRole('button', { name: 'Detect games' }));
      await world.user.click(await screen.findByRole('button', { name: 'Use this' }));
      await settled(world.controller);
    });

    Then('the game configuration is persisted', () => {
      expect(world.gateway.callsOf('setGameDir').map((call) => call.args)).toEqual([[steamRoot]]);
      expect(world.gateway.config.game_dir).toBe(steamRoot);
    });

    And('the current game is updated on screen', () => {
      // The Settings details; the status bar also names the game.
      const settings = within(screen.getByRole('main'));
      expect(settings.getByText('MotoGP 25')).toBeInTheDocument();
      expect(settings.getByText('1234')).toBeInTheDocument();
      expect(within(screen.getByRole('contentinfo')).getByText('MotoGP 25')).toBeInTheDocument();
    });
  });

  Scenario('Import through the dialog without duplicating', ({ Given, When, Then }) => {
    let world: World;

    Given('a test pak not yet in the library', async () => {
      world = await launch();
    });

    When('I select it in Import', async () => {
      world.picker.queueModFiles(['C:/mods/Cool.pak']);
      await world.user.click(screen.getByRole('button', { name: 'Import mods' }));
    });

    Then('the library shows its name, size and file count', async () => {
      const row = (await screen.findByText('Cool')).closest('li') as HTMLElement;
      expect(within(row).getByText('1 KB')).toBeInTheDocument();
      expect(within(row).getByText('1 file')).toBeInTheDocument();
    });

    When('I import the same pak again', async () => {
      world.picker.queueModFiles(['C:/mods/Cool.pak']);
      await world.user.click(screen.getByRole('button', { name: 'Import mods' }));
      await waitFor(() => expect(world.gateway.callsOf('importMods')).toHaveLength(2));
      await settled(world.controller);
    });

    Then('there is still a single entry for the mod', () => {
      expect(screen.getAllByText('Cool')).toHaveLength(1);
      expect(world.controller.getState().mods).toHaveLength(1);
    });
  });

  Scenario('Import by drag and drop', ({ Given, When, Then, And }) => {
    let world: World;

    Given('the Library screen open', async () => {
      world = await launch();
    });

    When('I receive a drop event with supported files', () => {
      act(() => world.drop.emitDrop(['C:/drop/Dropped.pak']));
    });

    Then('the paths are sent to the import use case', async () => {
      expect(await screen.findByText('Dropped')).toBeInTheDocument();
      expect(world.gateway.callsOf('importMods').map((call) => call.args)).toEqual([[['C:/drop/Dropped.pak']]]);
    });

    And('the list is reloaded', () => {
      expect(world.gateway.callsOf('listMods').length).toBeGreaterThanOrEqual(2);
    });
  });

  Scenario('Cancel the selection and remove the listener', ({ Given, When, Then, And }) => {
    let world: World;

    Given('an open dialog and active listeners on the screen', async () => {
      world = await launch({}, true);
      world.picker.queueModFiles(null);

      // StrictMode mounts twice; still exactly one callback per port.
      expect(world.session.activeListeners()).toBe(2);
      expect(world.drop.activeListeners()).toBe(1);
    });

    When('I cancel the dialog and then unmount the screen', async () => {
      await world.user.click(screen.getByRole('button', { name: 'Import mods' }));
      await settled(world.controller);
      expect(world.picker.modPicks).toBe(1);
      world.unmount();
    });

    Then('no import is requested', () => {
      expect(world.gateway.callsOf('importMods')).toHaveLength(0);
    });

    And('the listeners are released without duplicated callbacks', async () => {
      await waitFor(() => expect(world.session.activeListeners()).toBe(0));
      expect(world.drop.activeListeners()).toBe(0);
    });
  });

  Scenario('Create a profile and order mods', ({ Given, When, Then, And }) => {
    let world: World;

    Given('mods A and B in the library', async () => {
      world = await launch({ mods: [modA, modB] });
    });

    When('I create the profile "Teste", check the mods and move B before A', async () => {
      await openScreen(world.user, 'Profiles');
      await world.user.type(screen.getByLabelText('Profile name'), 'Teste');
      await world.user.click(screen.getByRole('button', { name: 'Create profile' }));
      await world.user.click(await screen.findByRole('button', { name: 'Teste', pressed: false }));

      await world.user.click(screen.getByRole('checkbox', { name: 'Mod A' }));
      await world.user.click(screen.getByRole('checkbox', { name: 'Mod B' }));
      await world.user.click(screen.getByRole('button', { name: 'Move Mod B up' }));
      await world.user.click(screen.getByRole('button', { name: 'Save' }));
    });

    Then('I save mod_ids in the order B, A', async () => {
      await waitFor(() => expect(world.gateway.callsOf('setProfileMods')).toHaveLength(1));
      await settled(world.controller);
      expect(world.gateway.callsOf('setProfileMods')[0]?.args).toEqual(['profile-1', ['B', 'A']]);
      expect(world.gateway.profiles.find((profile) => profile.name === 'Teste')?.mod_ids).toEqual(['B', 'A']);
    });

    And('the order remains after reloading the profile', async () => {
      // Reload: a brand new controller and UI over the same stored data.
      world.unmount();
      cleanup();
      const reloaded = await launch({
        mods: world.gateway.mods,
        profiles: world.gateway.profiles,
        config: world.gateway.config,
      });
      await openScreen(reloaded.user, 'Profiles');
      await reloaded.user.click(await screen.findByRole('button', { name: 'Teste' }));
      const order = screen
        .getAllByRole('button', { name: /^Move .* up$/ })
        .map((button) => button.getAttribute('aria-label'));
      expect(order).toEqual(['Move Mod B up', 'Move Mod A up']);
    });
  });

  Scenario('Rename, delete and protect profiles', ({ Given, When, Then, And }) => {
    let world: World;

    Given('a regular profile and Vanilla', async () => {
      world = await launch({
        profiles: [
          { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
          { id: 'p1', name: 'Teste', mod_ids: [] },
        ],
      });
      await openScreen(world.user, 'Profiles');
    });

    When('I rename the regular profile and then delete it', async () => {
      await world.user.click(screen.getByRole('button', { name: 'Teste' }));
      const input = screen.getByLabelText('New name');
      await world.user.clear(input);
      await world.user.type(input, 'Renomeado');
      await world.user.click(screen.getByRole('button', { name: 'Rename' }));

      const renamed = await screen.findByRole('button', { name: 'Renomeado' });
      expect(screen.queryByRole('button', { name: 'Teste' })).not.toBeInTheDocument();
      expect(world.gateway.profiles.map((profile) => profile.name)).toEqual(['Vanilla', 'Renomeado']);

      await world.user.click(renamed);
      await world.user.click(screen.getByRole('button', { name: 'Delete Renomeado' }));
    });

    Then('the list reflects both changes', async () => {
      await waitFor(() => expect(screen.queryByRole('button', { name: 'Renomeado' })).not.toBeInTheDocument());
      expect(world.gateway.profiles.map((profile) => profile.name)).toEqual(['Vanilla']);
      expect(screen.getByRole('button', { name: 'Vanilla' })).toBeInTheDocument();
    });

    And('Vanilla offers no delete action', async () => {
      await world.user.click(screen.getByRole('button', { name: 'Vanilla' }));
      expect(screen.queryByRole('button', { name: /^Delete/ })).not.toBeInTheDocument();
    });
  });

  Scenario('Remove a mod from the library and from profiles', ({ Given, When, Then }) => {
    let world: World;

    Given('a listed mod referenced in a profile', async () => {
      world = await launch({
        mods: [modA, modB],
        profiles: [
          { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
          { id: 'p1', name: 'Teste', mod_ids: ['A'] },
        ],
      });
    });

    When('I remove it from the Library', async () => {
      await world.user.click(screen.getByRole('button', { name: 'Remove Mod A' }));
    });

    Then('it no longer appears in the library nor in the profile selection', async () => {
      await waitFor(() => expect(screen.queryByText('Mod A')).not.toBeInTheDocument());
      await settled(world.controller);
      expect(screen.getByText('Mod B')).toBeInTheDocument();

      await openScreen(world.user, 'Profiles');
      await world.user.click(screen.getByRole('button', { name: 'Teste' }));
      expect(screen.queryByRole('checkbox', { name: 'Mod A' })).not.toBeInTheDocument();
      expect(screen.getByRole('checkbox', { name: 'Mod B' })).toBeInTheDocument();
      expect(world.gateway.profiles.find((profile) => profile.id === 'p1')?.mod_ids).toEqual([]);
    });
  });

  Scenario('Warn of an error when playing without an App ID', ({ Given, When, Then, And }) => {
    let world: World;

    Given('the profile "Teste" active and a game configured without an App ID', async () => {
      world = await launch({
        game: noAppIdGame,
        profiles: [
          { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
          { id: 'p1', name: 'Teste', mod_ids: ['A'] },
        ],
        config: { game_dir: fakeRoot, active_profile: 'p1' },
        mods: [modA],
      });
    });

    When('I click Play', async () => {
      await world.user.click(playButton());
    });

    Then('I see a notice containing "game App ID not found"', async () => {
      expect(await screen.findByRole('alert')).toHaveTextContent(/game App ID not found/);
    });

    And('no file is created in ~mods', () => {
      expect(world.gateway.callsOf('play').map((call) => call.args)).toEqual([['p1']]);
      // `~mods` stays untouched: the refusal happens before any deployment (Rust test in #8).
      expect(world.gateway.deployed).toEqual([]);
    });
  });

  Scenario('Control Play availability', ({ Given, When, Then }) => {
    let world: World;

    Given('the app without a configured game', async () => {
      world = await launch({ games: { [steamRoot]: { app_id: 1234 } } });
    });

    Then('Play is disabled', () => {
      expect(playButton()).toBeDisabled();
    });

    When('I configure a game and request to start a session', async () => {
      world.picker.queueDirectory(steamRoot);
      await openScreen(world.user, 'Settings');
      await world.user.click(screen.getByRole('button', { name: 'Choose folder…' }));
      await waitFor(() => expect(playButton()).toBeEnabled());

      await world.user.click(playButton());
    });

    Then('Play stays disabled during start and execution', async () => {
      expect(playButton()).toBeDisabled();
      expect(await screen.findByText('Starting…')).toBeInTheDocument();

      act(() => world.session.emitSession({ state: 'started', cleaned: 0 }));
      expect(await screen.findByText('Running…')).toBeInTheDocument();
      expect(playButton()).toBeDisabled();
    });
  });

  const requestedSession = async (): Promise<World> => {
    const world = await launch({ game: steamGame, config: { game_dir: steamRoot, active_profile: 'vanilla' } });
    await world.user.click(playButton());
    await waitFor(() => expect(world.controller.getState().session.kind).toBe('starting'));
    return world;
  };

  Scenario('Show session events', ({ Given, When, Then, And }) => {
    let world: World;

    Given('a requested session', async () => {
      world = await requestedSession();
    });

    When('I receive session with state "started"', () => {
      act(() => world.session.emitSession({ state: 'started', cleaned: 0 }));
    });

    Then('I see "Running…"', async () => {
      expect(await screen.findByText('Running…')).toBeInTheDocument();
    });

    When('I receive session with state "ended" and cleaned 2', () => {
      act(() => world.session.emitSession({ state: 'ended', cleaned: 2 }));
    });

    Then('I see "Session ended — 2 files removed"', async () => {
      expect(await screen.findByText('Session ended — 2 files removed')).toBeInTheDocument();
    });

    And('Play becomes available again', () => {
      expect(playButton()).toBeEnabled();
    });
  });

  Scenario('Show that the game did not open', ({ Given, When, Then }) => {
    let world: World;

    Given('a requested session', async () => {
      world = await requestedSession();
    });

    When('I receive session with state "never-started" after the cleanup finishes', () => {
      act(() => world.session.emitSession({ state: 'never-started', cleaned: 2 }));
    });

    Then('I see "The game did not open within 2 min; mods removed"', async () => {
      expect(await screen.findByText('The game did not open within 2 min; mods removed')).toBeInTheDocument();
      expect(playButton()).toBeEnabled();
    });
  });

  Scenario('Show operation errors', ({ Given, When, Then, And }) => {
    let world: World;

    Given('an operation whose port returns an error', async () => {
      world = await launch();
      world.gateway.failNext('importMods', { code: 'unsupported_format', message: 'bad file' });
      world.picker.queueModFiles(['C:/mods/Broken.pak']);
    });

    When('the operation finishes', async () => {
      await world.user.click(screen.getByRole('button', { name: 'Import mods' }));
    });

    Then('I see the error in the accessible notice at the top', async () => {
      const alert = await screen.findByRole('alert');
      expect(alert).toHaveTextContent('Unsupported file format.');
    });

    And('I do not see a false success confirmation', () => {
      expect(screen.queryByRole('status')).not.toBeInTheDocument();
    });
  });
});

// Not a Gherkin scenario: the start-up ordering contract of the controller, checked end to end.
describe('Given the launcher starts', () => {
  afterEach(cleanup);

  it('When it mounts Then session events are subscribed before recovery_status is requested', async () => {
    const world = await launch();

    const order = world.log;
    const recovery = order.indexOf('gateway.recoveryStatus');
    const sessionSub = order.indexOf('subscribe.session');
    const errorSub = order.indexOf('subscribe.session-error');
    expect(recovery).toBeGreaterThanOrEqual(0);
    expect(sessionSub).toBeGreaterThanOrEqual(0);
    expect(errorSub).toBeGreaterThanOrEqual(0);
    expect(sessionSub).toBeLessThan(recovery);
    expect(errorSub).toBeLessThan(recovery);
  });
});
