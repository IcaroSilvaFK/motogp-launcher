// End-to-end scenarios of tests/features/frontend/interface.feature: the real application
// controller and the real UI, with only the ports replaced by the in-memory fakes.
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

afterEach(cleanup);

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

describe('Given the launcher starts', () => {
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

describe('Given the fake folder "C:/temp/MotoGPFake/MotoGPFake/Content/Paks"', () => {
  it('When I choose "C:/temp/MotoGPFake" in Settings Then I see mods_dir ending in "Content/Paks/~mods" and the App ID as not found', async () => {
    const world = await launch();
    world.picker.queueDirectory(fakeRoot);

    await openScreen(world.user, 'Settings');
    await world.user.click(screen.getByRole('button', { name: 'Choose folder…' }));

    const modsDir = await screen.findByText(/Content\/Paks\/~mods$/);
    expect(modsDir).toHaveTextContent('C:/temp/MotoGPFake/MotoGPFake/Content/Paks/~mods');
    expect(screen.getByText('not found')).toBeInTheDocument();
  });
});

describe('Given detection returns a MotoGP with an App ID', () => {
  it('When I click "Use this" Then the game configuration is persisted and the current game is updated on screen', async () => {
    const world = await launch({ detected: [steamGame], games: { [steamRoot]: steamGame } });

    await openScreen(world.user, 'Settings');
    await world.user.click(screen.getByRole('button', { name: 'Detect games' }));
    await world.user.click(await screen.findByRole('button', { name: 'Use this' }));

    await settled(world.controller);
    expect(world.gateway.callsOf('setGameDir').map((call) => call.args)).toEqual([[steamRoot]]);
    expect(world.gateway.config.game_dir).toBe(steamRoot);
    expect(screen.getByText('MotoGP 25')).toBeInTheDocument();
    expect(screen.getByText('1234')).toBeInTheDocument();
  });
});

describe('Given a test pak not yet in the library', () => {
  it('When I select it in Import and import it again Then the library shows name, size and file count with a single entry', async () => {
    const world = await launch();
    world.picker.queueModFiles(['C:/mods/Cool.pak']);

    await world.user.click(screen.getByRole('button', { name: 'Import mods' }));
    const row = (await screen.findByText('Cool')).closest('tr') as HTMLElement;
    expect(within(row).getByText('1 KB')).toBeInTheDocument();
    expect(within(row).getByText('1 file')).toBeInTheDocument();

    world.picker.queueModFiles(['C:/mods/Cool.pak']);
    await world.user.click(screen.getByRole('button', { name: 'Import mods' }));
    await waitFor(() => expect(world.gateway.callsOf('importMods')).toHaveLength(2));
    await settled(world.controller);

    expect(screen.getAllByText('Cool')).toHaveLength(1);
    expect(world.controller.getState().mods).toHaveLength(1);
  });
});

describe('Given the Library screen open', () => {
  it('When I receive a drop event with supported files Then the paths are imported and the list is reloaded', async () => {
    const world = await launch();

    act(() => world.drop.emitDrop(['C:/drop/Dropped.pak']));

    expect(await screen.findByText('Dropped')).toBeInTheDocument();
    expect(world.gateway.callsOf('importMods').map((call) => call.args)).toEqual([[['C:/drop/Dropped.pak']]]);
    expect(world.gateway.callsOf('listMods').length).toBeGreaterThanOrEqual(2);
  });
});

describe('Given an open dialog and active listeners on the screen', () => {
  it('When I cancel the dialog and then unmount Then no import is requested and the listeners are released without duplicates', async () => {
    const world = await launch({}, true);
    world.picker.queueModFiles(null);

    // StrictMode mounts twice; still exactly one callback per port.
    expect(world.session.activeListeners()).toBe(2);
    expect(world.drop.activeListeners()).toBe(1);

    await world.user.click(screen.getByRole('button', { name: 'Import mods' }));
    await settled(world.controller);
    expect(world.picker.modPicks).toBe(1);
    expect(world.gateway.callsOf('importMods')).toHaveLength(0);

    world.unmount();

    await waitFor(() => expect(world.session.activeListeners()).toBe(0));
    expect(world.drop.activeListeners()).toBe(0);
  });
});

describe('Given mods A and B in the library', () => {
  it('When I create the profile "Teste", check the mods and move B before A Then mod_ids are saved as B, A and stay in that order after reloading', async () => {
    const world = await launch({ mods: [modA, modB] });

    await openScreen(world.user, 'Profiles');
    await world.user.type(screen.getByLabelText('Profile name'), 'Teste');
    await world.user.click(screen.getByRole('button', { name: 'Create profile' }));
    await world.user.click(await screen.findByRole('button', { name: 'Teste', pressed: false }));

    await world.user.click(screen.getByRole('checkbox', { name: 'Mod A' }));
    await world.user.click(screen.getByRole('checkbox', { name: 'Mod B' }));
    await world.user.click(screen.getByRole('button', { name: 'Move Mod B up' }));
    await world.user.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => expect(world.gateway.callsOf('setProfileMods')).toHaveLength(1));
    await settled(world.controller);
    expect(world.gateway.callsOf('setProfileMods')[0]?.args).toEqual(['profile-1', ['B', 'A']]);
    expect(world.gateway.profiles.find((profile) => profile.name === 'Teste')?.mod_ids).toEqual(['B', 'A']);

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

describe('Given a regular profile and Vanilla', () => {
  const profiles = [
    { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
    { id: 'p1', name: 'Teste', mod_ids: [] },
  ];

  it('When I rename the regular profile and then delete it Then the list reflects both changes and Vanilla offers no delete action', async () => {
    const world = await launch({ profiles });
    await openScreen(world.user, 'Profiles');

    await world.user.click(screen.getByRole('button', { name: 'Vanilla' }));
    expect(screen.queryByRole('button', { name: /^Delete/ })).not.toBeInTheDocument();

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

    await waitFor(() => expect(screen.queryByRole('button', { name: 'Renomeado' })).not.toBeInTheDocument());
    expect(world.gateway.profiles.map((profile) => profile.name)).toEqual(['Vanilla']);
    expect(screen.getByRole('button', { name: 'Vanilla' })).toBeInTheDocument();
  });
});

describe('Given a listed mod referenced in a profile', () => {
  it('When I remove it from the Library Then it no longer appears in the library nor in the profile selection', async () => {
    const world = await launch({
      mods: [modA, modB],
      profiles: [
        { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
        { id: 'p1', name: 'Teste', mod_ids: ['A'] },
      ],
    });

    await world.user.click(screen.getByRole('button', { name: 'Remove Mod A' }));
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

describe('Given the profile "Teste" active and a game configured without an App ID', () => {
  it('When I click Play Then I see a notice containing "game App ID not found" and no file is created in ~mods', async () => {
    const world = await launch({
      game: noAppIdGame,
      profiles: [
        { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
        { id: 'p1', name: 'Teste', mod_ids: ['A'] },
      ],
      config: { game_dir: fakeRoot, active_profile: 'p1' },
      mods: [modA],
    });

    await world.user.click(playButton());

    expect(await screen.findByRole('alert')).toHaveTextContent(/game App ID not found/);
    expect(world.gateway.callsOf('play').map((call) => call.args)).toEqual([['p1']]);
    // `~mods` stays untouched: the refusal happens before any deployment (Rust test in #8).
    expect(world.gateway.deployed).toEqual([]);
  });
});

describe('Given the app without a configured game', () => {
  it('When I configure a game and request to start a session Then Play is disabled before, and stays disabled during start and execution', async () => {
    const world = await launch({ games: { [steamRoot]: { app_id: 1234 } } });
    expect(playButton()).toBeDisabled();

    world.picker.queueDirectory(steamRoot);
    await openScreen(world.user, 'Settings');
    await world.user.click(screen.getByRole('button', { name: 'Choose folder…' }));
    await waitFor(() => expect(playButton()).toBeEnabled());

    await world.user.click(playButton());
    expect(playButton()).toBeDisabled();
    expect(await screen.findByText('Starting…')).toBeInTheDocument();

    act(() => world.session.emitSession({ state: 'started', cleaned: 0 }));
    expect(await screen.findByText('Running…')).toBeInTheDocument();
    expect(playButton()).toBeDisabled();
  });
});

describe('Given a requested session', () => {
  const requested = async (): Promise<World> => {
    const world = await launch({ game: steamGame, config: { game_dir: steamRoot, active_profile: 'vanilla' } });
    await world.user.click(playButton());
    await waitFor(() => expect(world.controller.getState().session.kind).toBe('starting'));
    return world;
  };

  it('When I receive session "started" and then "ended" with cleaned 2 Then I see "Running…", then "Session ended — 2 files removed" and Play is available again', async () => {
    const world = await requested();

    act(() => world.session.emitSession({ state: 'started', cleaned: 0 }));
    expect(await screen.findByText('Running…')).toBeInTheDocument();

    act(() => world.session.emitSession({ state: 'ended', cleaned: 2 }));
    expect(await screen.findByText('Session ended — 2 files removed')).toBeInTheDocument();
    expect(playButton()).toBeEnabled();
  });

  it('When I receive session "never-started" after the cleanup finishes Then I see "The game did not open within 2 min; mods removed"', async () => {
    const world = await requested();

    act(() => world.session.emitSession({ state: 'never-started', cleaned: 2 }));
    expect(await screen.findByText('The game did not open within 2 min; mods removed')).toBeInTheDocument();
    expect(playButton()).toBeEnabled();
  });
});

describe('Given an operation whose port returns an error', () => {
  it('When the operation finishes Then I see the error in the accessible notice at the top and no false success confirmation', async () => {
    const world = await launch();
    world.gateway.failNext('importMods', { code: 'unsupported_format', message: 'bad file' });
    world.picker.queueModFiles(['C:/mods/Broken.pak']);

    await world.user.click(screen.getByRole('button', { name: 'Import mods' }));

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('Unsupported file format.');
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });
});
