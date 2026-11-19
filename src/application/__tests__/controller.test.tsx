import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { CommandError, GameInfo, ModEntry, Profile } from '../../domain';
import { VANILLA_ID } from '../../domain';
import { createLauncherController } from '../createLauncherController';
import { selectCanPlay } from '../selectCanPlay';
import { useLauncherState } from '../useLauncherState';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from './fakes';
import type { FakeGatewayOptions, FakeSessionEventsOptions } from './fakes';

const game: GameInfo = { name: 'MotoGP25', root: 'C:/g', mods_dir: 'C:/g/~mods', app_id: 1234 };

function setup(options: FakeGatewayOptions = {}, sessionOptions: FakeSessionEventsOptions = {}) {
  const log: string[] = [];
  const gateway = new FakeGateway({ log, game, ...options });
  const picker = new FakePicker();
  const session = new FakeSessionEvents({ log, ...sessionOptions });
  const drop = new FakeDropEvents({ log });
  const controller = createLauncherController({ gateway, picker, session, drop });
  return { log, gateway, picker, session, drop, controller };
}

async function ready(options: FakeGatewayOptions = {}) {
  const ctx = setup(options);
  await ctx.controller.start();
  return ctx;
}

const mod = (id: string): ModEntry => ({ id, name: id, files: [`${id}.pak`], size_bytes: 1 });

describe('Given the launcher controller', () => {
  it('When started Then loads_launcher_state', async () => {
    const { controller, gateway } = setup({ mods: [mod('a')] });
    expect(controller.getState().status).toBe('loading');
    await controller.start();
    const state = controller.getState();
    expect(state.status).toBe('ready');
    expect(state.game).toEqual(game);
    expect(state.mods).toHaveLength(1);
    expect(state.profiles[0]?.id).toBe(VANILLA_ID);
    expect(state.config.active_profile).toBe(VANILLA_ID);
    expect(state.recovery?.state).toBe('not-run');
    expect(gateway.callsOf('getConfig')).toHaveLength(1);
  });

  it('When started Then subscribes_before_recovery_status', async () => {
    const { controller, log } = setup();
    await controller.start();
    const rec = log.indexOf('gateway.recoveryStatus');
    const sessionSub = log.indexOf('subscribe.session');
    const errorSub = log.indexOf('subscribe.session-error');
    expect(rec).toBeGreaterThanOrEqual(0);
    expect(sessionSub).toBeGreaterThanOrEqual(0);
    expect(errorSub).toBeGreaterThanOrEqual(0);
    expect(sessionSub).toBeLessThan(rec);
    expect(errorSub).toBeLessThan(rec);
  });

  it('When recovery reports states Then notices and errors are exposed (recovery_failure_is_exposed)', async () => {
    const failed = await ready({ recovery: { state: 'failed', error: 'boom', cleaned_count: 0 } });
    expect(failed.controller.getState().error).toEqual({ code: 'recovery_failed', message: 'boom' });
    expect(failed.controller.getState().status).toBe('ready');

    const noMsg = await ready({ recovery: { state: 'failed', error: null, cleaned_count: 0 } });
    expect(noMsg.controller.getState().error?.message).toBe('');

    const rec = await ready({ recovery: { state: 'recovered', error: null, cleaned_count: 3 } });
    expect(rec.controller.getState().notice).toEqual({ kind: 'recovered', cleaned: 3 });

    const none = await ready({ recovery: { state: 'recovered', error: null, cleaned_count: 0 } });
    expect(none.controller.getState().notice).toBeNull();

    const running = await ready({ recovery: { state: 'game-running', error: null, cleaned_count: 0 } });
    expect(running.controller.getState().notice).toEqual({ kind: 'game-running' });
  });

  it('When a load fails Then error is exposed and status is ready', async () => {
    const { controller, gateway } = setup();
    gateway.failNext('listMods', { code: 'read_failed', message: 'x' });
    await controller.start();
    expect(controller.getState().error?.code).toBe('read_failed');
    expect(controller.getState().status).toBe('ready');
  });

  it('When the dialog picks files Then imports_and_refreshes_library', async () => {
    const { controller, picker, gateway } = await ready();
    picker.queueModFiles(['C:/x/a.pak', 'C:/x/b.zip', 'C:/x/readme.txt']);
    await controller.importFromDialog();
    expect(gateway.callsOf('importMods')[0]?.args).toEqual([['C:/x/a.pak', 'C:/x/b.zip']]);
    const state = controller.getState();
    expect(state.mods.map((m) => m.id)).toEqual(['a', 'b']);
    expect(state.notice).toEqual({ kind: 'imported', count: 2 });
    expect(state.busy).toBe(false);
  });

  it('When a batch import fails Then failed_batch_import_still_reloads', async () => {
    const { controller, gateway } = await ready();
    const before = gateway.callsOf('listMods').length;
    gateway.failNext('importMods', { code: 'mod_without_pak', message: 'bad' });
    await controller.importPaths(['C:/x/a.pak']);
    expect(gateway.callsOf('listMods').length).toBe(before + 1);
    expect(controller.getState().error?.code).toBe('mod_without_pak');
    expect(controller.getState().notice).toBeNull();
    expect(controller.getState().busy).toBe(false);
  });

  it('When the same mod is imported twice Then reimport_keeps_single_mod', async () => {
    const { controller } = await ready();
    await controller.importPaths(['C:/x/a.pak']);
    await controller.importPaths(['C:/x/a.pak']);
    expect(controller.getState().mods).toHaveLength(1);
  });

  it('When the dialog is cancelled Then cancelled_dialog_does_not_import', async () => {
    const { controller, picker, gateway } = await ready();
    picker.queueModFiles(null);
    await controller.importFromDialog();
    expect(gateway.callsOf('importMods')).toHaveLength(0);
    picker.queueDirectory(null);
    await controller.chooseGameDirectory();
    expect(gateway.callsOf('setGameDir')).toHaveLength(0);
  });

  it('When no supported path is given Then nothing is imported', async () => {
    const { controller, gateway } = await ready();
    await controller.importPaths(['C:/x/readme.txt']);
    expect(gateway.callsOf('importMods')).toHaveLength(0);
  });

  it('When files are dropped Then drop_imports_supported_paths', async () => {
    const { controller, drop, gateway } = await ready();
    const release = await controller.subscribeDrop();
    drop.emitDrop(['C:/x/a.pak', 'C:/x/notes.txt']);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 5));
    });
    expect(gateway.callsOf('importMods')[0]?.args).toEqual([['C:/x/a.pak']]);
    release();
    expect(drop.activeListeners()).toBe(0);
  });

  it('When a mod is removed Then remove_mod_refreshes_profiles', async () => {
    const profiles: Profile[] = [
      { id: VANILLA_ID, name: 'Vanilla', mod_ids: [] },
      { id: 'p', name: 'P', mod_ids: ['a'] },
    ];
    const { controller } = await ready({ mods: [mod('a')], profiles });
    await controller.removeMod('a');
    const state = controller.getState();
    expect(state.mods).toHaveLength(0);
    expect(state.profiles.find((p) => p.id === 'p')?.mod_ids).toEqual([]);
    expect(state.notice).toEqual({ kind: 'mod-removed' });
  });

  it('When profile mods are saved Then saves_profile_mod_order', async () => {
    const { controller } = await ready({ mods: [mod('a'), mod('b')] });
    await controller.createProfile('Mine');
    const id = controller.getState().profiles.find((p) => p.name === 'Mine')?.id ?? '';
    await controller.setProfileMods(id, ['b', 'a']);
    expect(controller.getState().notice).toEqual({ kind: 'profile-saved' });
    expect(controller.getState().profiles.find((p) => p.id === id)?.mod_ids).toEqual(['b', 'a']);
    await controller.renameProfile(id, 'Renamed');
    expect(controller.getState().profiles.find((p) => p.id === id)?.name).toBe('Renamed');
    expect(controller.getState().profiles.find((p) => p.id === id)?.mod_ids).toEqual(['b', 'a']);
  });

  it('When the active profile changes or is deleted Then sets_active_profile', async () => {
    const { controller, gateway } = await ready();
    await controller.createProfile('Mine');
    const id = controller.getState().profiles.find((p) => p.name === 'Mine')?.id ?? '';
    await controller.setActiveProfile(id);
    expect(controller.getState().config.active_profile).toBe(id);
    const before = gateway.callsOf('getConfig').length;
    await controller.deleteProfile(id);
    expect(gateway.callsOf('getConfig').length).toBe(before + 1);
    expect(controller.getState().profiles.some((p) => p.id === id)).toBe(false);
  });

  it('When a game is chosen Then game is configured and detection works', async () => {
    const { controller, picker } = await ready({ game: null, detected: [game] });
    await controller.detectGames();
    expect(controller.getState().detected).toEqual([game]);
    picker.queueDirectory('C:/Jogos/MotoGP');
    await controller.chooseGameDirectory();
    expect(controller.getState().game?.root).toBe('C:/Jogos/MotoGP');
    expect(controller.getState().notice).toEqual({ kind: 'game-configured', name: 'MotoGP' });
    expect(controller.getState().config.game_dir).toBe('C:/Jogos/MotoGP');
  });

  it('When play has no App ID Then play_refuses_without_app_id', async () => {
    const { controller, gateway } = await ready({ game: { ...game, app_id: null } });
    await controller.play();
    expect(controller.getState().error?.code).toBe('missing_app_id');
    expect(controller.getState().error?.message).toContain('App ID');
    expect(controller.getState().session).toEqual({ kind: 'idle' });
    expect(gateway.deployed).toEqual([]);
  });

  it('When session events arrive Then handles_session_events', async () => {
    const { controller, session, gateway } = await ready();
    await controller.play();
    expect(gateway.callsOf('play')[0]?.args).toEqual([VANILLA_ID]);
    expect(controller.getState().session).toEqual({ kind: 'starting' });
    session.emitSession({ state: 'started', cleaned: 0 });
    expect(controller.getState().session).toEqual({ kind: 'running' });
    session.emitSession({ state: 'ended', cleaned: 4 });
    expect(controller.getState().session).toEqual({ kind: 'ended', cleaned: 4 });
    session.emitSession({ state: 'never-started', cleaned: 2 });
    expect(controller.getState().session).toEqual({ kind: 'never-started', cleaned: 2 });
  });

  it('When a session error arrives Then session_error_does_not_announce_cleanup', async () => {
    const { controller, session } = await ready();
    await controller.play();
    session.emitSessionError('watch failed');
    const state = controller.getState();
    expect(state.error).toEqual({ code: 'session_error', message: 'watch failed' });
    expect(state.session).toEqual({ kind: 'idle' });
    expect(state.notice).toBeNull();
  });

  it('When a mutation fails Then operation_error_is_exposed without success notice', async () => {
    const { controller, gateway } = await ready();
    gateway.failNext('createProfile', { code: 'name_collision', message: 'dup' });
    await controller.createProfile('x');
    expect(controller.getState().error).toEqual({ code: 'name_collision', message: 'dup' });
    expect(controller.getState().notice).toBeNull();
    gateway.failNext('removeMod', new Error('plain') as unknown as CommandError);
    await controller.removeMod('a');
    expect(controller.getState().error?.code).toBe('unknown');
    expect(controller.getState().notice).toBeNull();
    controller.dismissError();
    expect(controller.getState().error).toBeNull();
    await controller.importPaths(['C:/x/a.pak']);
    expect(controller.getState().notice).not.toBeNull();
    controller.dismissNotice();
    expect(controller.getState().notice).toBeNull();
  });

  it('When disposed Then listeners_are_released_on_dispose (also on double start)', async () => {
    const { controller, session } = setup();
    await Promise.all([controller.start(), controller.start()]);
    expect(session.activeListeners()).toBe(2);
    controller.dispose();
    controller.dispose();
    expect(session.activeListeners()).toBe(0);

    const early = setup();
    const pending = early.controller.start();
    early.controller.dispose();
    await pending;
    expect(early.session.activeListeners()).toBe(0);
  });

  it('When started again after dispose (StrictMode remount) Then exactly one set of listeners is active', async () => {
    const { controller, session } = setup();
    const first = controller.start();
    controller.dispose();
    const second = controller.start();
    await Promise.all([first, second]);
    expect(session.activeListeners()).toBe(2);
    controller.dispose();
    expect(session.activeListeners()).toBe(0);
  });

  it('When the session subscription rejects Then the session is unavailable and the error is reported', async () => {
    const { controller, gateway } = setup(
      { mods: [mod('a')] },
      { rejectSession: { code: 'unknown', message: 'no ipc' } },
    );
    await controller.start();
    const state = controller.getState();
    expect(state.error).toEqual({ code: 'session_subscription_failed', message: 'no ipc' });
    expect(state.session).toEqual({ kind: 'unavailable' });
    expect(state.status).toBe('ready');
    expect(state.mods).toHaveLength(1);
    expect(state.game).toEqual(game);
    expect(gateway.callsOf('recoveryStatus')).toHaveLength(1);
    expect(selectCanPlay(state)).toBe(false);
    await controller.play();
    expect(gateway.callsOf('play')).toHaveLength(0);
  });

  it('When a non-Error reason rejects Then the message comes from toCommandError', async () => {
    const { controller } = setup({}, { rejectSession: 'plain string' });
    await controller.start();
    expect(controller.getState().error?.code).toBe('session_subscription_failed');
    expect(controller.getState().error?.message).toBe('plain string');
  });

  it('When the second subscription rejects Then the first one is released', async () => {
    const { controller, session } = setup({}, { rejectSessionError: new Error('late') });
    await controller.start();
    expect(session.activeListeners()).toBe(0);
    expect(controller.getState().error).toEqual({ code: 'session_subscription_failed', message: 'late' });
    expect(controller.getState().session).toEqual({ kind: 'unavailable' });
  });

  it('When the subscription and recovery both fail Then the subscription error wins', async () => {
    const { controller } = setup(
      { recovery: { state: 'failed', error: 'boom', cleaned_count: 0 } },
      { rejectSession: new Error('no ipc') },
    );
    await controller.start();
    expect(controller.getState().error?.code).toBe('session_subscription_failed');
  });

  it('When disposed during a failing start Then no stale state is written', async () => {
    const { controller } = setup({}, { rejectSession: new Error('no ipc') });
    const pending = controller.start();
    controller.dispose();
    await pending;
    expect(controller.getState().status).toBe('loading');
    expect(controller.getState().error).toBeNull();
    expect(controller.getState().session).toEqual({ kind: 'idle' });
  });

  it('When restarted after a failure (StrictMode remount) Then a fresh run is started', async () => {
    const { controller, gateway } = setup({}, { rejectSession: new Error('no ipc') });
    await controller.start();
    controller.dispose();
    await controller.start();
    expect(gateway.callsOf('recoveryStatus')).toHaveLength(2);
  });

  it('When state changes Then subscribers are notified and state is stable', async () => {
    const { controller } = setup();
    let calls = 0;
    const off = controller.subscribe(() => (calls += 1));
    const first = controller.getState();
    expect(controller.getState()).toBe(first);
    await controller.start();
    expect(calls).toBeGreaterThan(0);
    off();
    const seen = calls;
    controller.dismissNotice();
    await controller.createProfile('z');
    expect(calls).toBe(seen);
  });
});

describe('Given selectCanPlay', () => {
  it('When conditions vary Then can_play_rules', async () => {
    const { controller } = await ready();
    const base = controller.getState();
    expect(selectCanPlay(base)).toBe(true);
    expect(selectCanPlay({ ...base, status: 'loading' })).toBe(false);
    expect(selectCanPlay({ ...base, game: null })).toBe(false);
    expect(selectCanPlay({ ...base, config: { ...base.config, active_profile: null } })).toBe(false);
    expect(selectCanPlay({ ...base, busy: true })).toBe(false);
    expect(selectCanPlay({ ...base, session: { kind: 'starting' } })).toBe(false);
    expect(selectCanPlay({ ...base, session: { kind: 'running' } })).toBe(false);
    expect(selectCanPlay({ ...base, session: { kind: 'ended', cleaned: 1 } })).toBe(true);
  });

  it('When play is refused by rules Then gateway is not called', async () => {
    const { controller, gateway } = setup({ game: null });
    await controller.play();
    expect(gateway.callsOf('play')).toHaveLength(0);
  });
});

describe('Given useLauncherState', () => {
  it('When the controller changes Then use_launcher_state_rerenders', async () => {
    const { controller } = setup();
    const { result } = renderHook(() => useLauncherState(controller));
    expect(result.current.status).toBe('loading');
    await act(async () => {
      await controller.start();
    });
    expect(result.current.status).toBe('ready');
  });
});
