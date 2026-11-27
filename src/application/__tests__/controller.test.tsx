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
    expect(state.notice).toEqual({ kind: 'imported', count: 2, rejected: ['readme.txt'] });
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
    expect(controller.getState().notice).toEqual({ kind: 'imported', count: 1, rejected: ['notes.txt'] });
    release();
    expect(drop.activeListeners()).toBe(0);
  });

  it('When only unsupported files are dropped Then a rejected notice names them', async () => {
    const { controller, drop, gateway } = await ready();
    await controller.subscribeDrop();
    drop.emitDrop(['C:\\x\\readme.txt', 'D:/y/photo.png']);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 5));
    });
    expect(gateway.callsOf('importMods')).toHaveLength(0);
    expect(controller.getState().notice).toEqual({ kind: 'files-rejected', names: ['readme.txt', 'photo.png'] });
  });

  it('When files are dragged Then dragging follows the drag state until drop or cancel', async () => {
    const { controller, drop } = await ready();
    const release = await controller.subscribeDrop();
    expect(controller.getState().dragging).toBe(false);
    drop.emitDragState(true);
    expect(controller.getState().dragging).toBe(true);
    drop.emitDragState(false);
    expect(controller.getState().dragging).toBe(false);
    drop.emitDragState(true);
    controller.cancelDrag();
    expect(controller.getState().dragging).toBe(false);
    // The SDK keeps sending "over" while the same drag continues: Esc wins until it ends.
    drop.emitDragState(true);
    expect(controller.getState().dragging).toBe(false);
    drop.emitDragState(false);
    drop.emitDragState(true);
    expect(controller.getState().dragging).toBe(true);
    drop.emitDrop(['C:/x/a.pak']);
    expect(controller.getState().dragging).toBe(false);
    drop.emitDragState(true);
    expect(drop.activeDragListeners()).toBe(1);
    release();
    expect(controller.getState().dragging).toBe(false);
    expect(drop.activeListeners()).toBe(0);
    expect(drop.activeDragListeners()).toBe(0);
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
    session.emitSession({ state: 'started', cleaned: 0, started_at: null });
    expect(controller.getState().session).toEqual({ kind: 'running', since: null });
    session.emitSession({ state: 'ended', cleaned: 4, started_at: null });
    expect(controller.getState().session).toEqual({ kind: 'ended', cleaned: 4 });
    session.emitSession({ state: 'never-started', cleaned: 2, started_at: null });
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
    expect(selectCanPlay({ ...base, session: { kind: 'running', since: null } })).toBe(false);
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

describe('Given a stale deploy session', () => {
  const OLD = 'D:/old/~mods';
  const stale = { mods_dir: OLD, tracked: 3 };

  it('Then the notice names the old folder and Play is disabled (play_blocked_while_stale)', async () => {
    const { controller, gateway } = await ready({ manifest: stale });
    expect(controller.getState().staleSession).toEqual({ modsDir: OLD, tracked: 3 });
    expect(selectCanPlay(controller.getState())).toBe(false);
    await controller.play();
    expect(gateway.callsOf('play')).toHaveLength(0);
  });

  it('When the user forgets the old session Then the notice goes away and Play is enabled (forget_stale_session)', async () => {
    const { controller, gateway } = await ready({ manifest: stale });
    await controller.forgetStaleSession();
    const state = controller.getState();
    expect(gateway.callsOf('forgetStaleSession')).toHaveLength(1);
    expect(state.staleSession).toBeNull();
    expect(state.notice).toEqual({ kind: 'stale-forgotten' });
    expect(state.error).toBeNull();
    expect(state.busy).toBe(false);
    expect(selectCanPlay(state)).toBe(true);
    await controller.play();
    expect(gateway.callsOf('play')).toHaveLength(1);
  });

  it('When the user cleans the old folder Then it sends the shown folder and the notice goes away (clean_stale_session)', async () => {
    const { controller, gateway } = await ready({ manifest: stale });
    await controller.cleanStaleSession();
    const state = controller.getState();
    expect(gateway.callsOf('cleanStaleSession')[0]?.args).toEqual([OLD]);
    expect(state.staleSession).toBeNull();
    expect(state.notice).toEqual({ kind: 'stale-cleaned', cleaned: 3 });
    expect(selectCanPlay(state)).toBe(true);
  });

  it('When cleaning is refused Then the error is shown and the stale notice stays (clean_refused_keeps_notice)', async () => {
    const { controller, gateway } = await ready({ manifest: stale });
    gateway.failNext('cleanStaleSession', { code: 'deploy_unauthorized_directory', message: 'no' });
    await controller.cleanStaleSession();
    const state = controller.getState();
    expect(state.error?.code).toBe('deploy_unauthorized_directory');
    expect(state.notice).toBeNull();
    expect(state.staleSession).toEqual({ modsDir: OLD, tracked: 3 });
    expect(state.busy).toBe(false);
    expect(selectCanPlay(state)).toBe(false);
  });

  it('When the user restores the game files Then the deployed count is reported and refreshed (restore_game_files)', async () => {
    const { controller, gateway } = await ready({ manifest: { mods_dir: game.mods_dir, tracked: 2 } });
    expect(controller.getState().deployedFiles).toBe(2);
    await controller.restoreGameFiles();
    const state = controller.getState();
    expect(gateway.callsOf('restoreGameFiles')).toHaveLength(1);
    expect(state.notice).toEqual({ kind: 'game-restored', cleaned: 2 });
    expect(state.deployedFiles).toBe(0);
    expect(state.busy).toBe(false);
  });

  it('When restoring is refused Then the error is shown and no notice is raised (restore_refused)', async () => {
    const { controller, gateway } = await ready({ manifest: { mods_dir: game.mods_dir, tracked: 2 } });
    gateway.failNext('restoreGameFiles', { code: 'game_still_running', message: 'no' });
    await controller.restoreGameFiles();
    const state = controller.getState();
    expect(state.error?.code).toBe('game_still_running');
    expect(state.notice).toBeNull();
    expect(state.deployedFiles).toBe(2);
  });

  it('When the manifest is stale or absent Then deployedFiles is 0 (deployed_files_current_only)', async () => {
    expect((await ready({ manifest: stale })).controller.getState().deployedFiles).toBe(0);
    expect((await ready({})).controller.getState().deployedFiles).toBe(0);
  });

  it('When a different folder is confirmed Then the backend refuses and nothing is removed (confirmed_dir_mismatch)', async () => {
    const { gateway } = await ready({ manifest: stale });
    await expect(gateway.cleanStaleSession('D:/other/~mods')).rejects.toMatchObject({
      code: 'deploy_unauthorized_directory',
    });
    expect(gateway.manifestState()).toBe('stale');
  });

  it('When the game folder is switched with a deployed session Then the stale notice appears (switch_game_makes_stale)', async () => {
    const { controller } = await ready({ manifest: { mods_dir: game.mods_dir, tracked: 2 } });
    expect(controller.getState().staleSession).toBeNull();
    await controller.useGame('C:/other');
    const state = controller.getState();
    expect(state.game?.root).toBe('C:/other');
    expect(state.staleSession).toEqual({ modsDir: game.mods_dir, tracked: 2 });
    expect(selectCanPlay(state)).toBe(false);
  });

  it('When the old folder vanished Then there is no notice and Play is enabled (vanished_needs_no_recovery)', async () => {
    const { controller } = await ready({ manifest: { ...stale, vanished: true } });
    expect(controller.getState().staleSession).toBeNull();
    expect(selectCanPlay(controller.getState())).toBe(true);
  });

  it('When there is no session or it is current Then staleSession is null', async () => {
    expect((await ready()).controller.getState().staleSession).toBeNull();
    const current = await ready({ manifest: { mods_dir: game.mods_dir, tracked: 1 } });
    expect(current.controller.getState().staleSession).toBeNull();
  });

  it('When startup recovery failed because of the stale manifest Then the generic error is dropped (startup_recovery_failed_suppressed)', async () => {
    const { controller } = await ready({
      manifest: stale,
      recovery: { state: 'failed', error: 'stale manifest', cleaned_count: 0 },
    });
    const state = controller.getState();
    expect(state.staleSession).not.toBeNull();
    expect(state.error).toBeNull();
    expect(state.recovery?.state).toBe('failed');
    await controller.forgetStaleSession();
    expect(selectCanPlay(controller.getState())).toBe(true);
  });

  it('When recovery failed with no stale session Then the error is still shown', async () => {
    const { controller } = await ready({ recovery: { state: 'failed', error: 'boom', cleaned_count: 0 } });
    expect(controller.getState().error?.code).toBe('recovery_failed');
  });

  it('When no game is configured Then deploySession is not called (no_game_no_backend_call)', async () => {
    const { controller, gateway } = await ready({ game: null, manifest: stale });
    expect(controller.getState().staleSession).toBeNull();
    expect(gateway.callsOf('deploySession')).toHaveLength(0);
  });

  it('When the current session is forgotten Then deploy_session_not_stale is returned (current_cannot_be_forgotten)', async () => {
    const { controller, gateway } = await ready({ manifest: { mods_dir: game.mods_dir, tracked: 1 } });
    expect(gateway.manifestState()).toBe('current');
    await expect(gateway.forgetStaleSession()).rejects.toMatchObject({ code: 'deploy_session_not_stale' });
    expect(gateway.manifestState()).toBe('current');
    // The controller itself is a no-op without a stale session.
    await controller.forgetStaleSession();
    await controller.cleanStaleSession();
    expect(gateway.callsOf('cleanStaleSession')).toHaveLength(0);
    expect(controller.getState().error).toBeNull();
  });

  it('When a session is active Then forget and clean errors are normalized and the notice stays (active_session_refuses)', async () => {
    const { controller, gateway } = await ready({ manifest: stale });
    gateway.failNext('forgetStaleSession', { code: 'already_running', message: 'running' });
    await controller.forgetStaleSession();
    expect(controller.getState().error?.code).toBe('already_running');
    expect(controller.getState().staleSession).not.toBeNull();
    gateway.failNext('cleanStaleSession', { code: 'game_still_running', message: 'running' });
    await controller.cleanStaleSession();
    expect(controller.getState().error?.code).toBe('game_still_running');
    expect(gateway.manifestState()).toBe('stale');
  });

  it('When the session query fails Then the error is exposed and the rest still loads', async () => {
    const { controller, gateway } = setup({ manifest: stale });
    gateway.failNext('deploySession', { code: 'read_failed', message: 'x' });
    await controller.start();
    expect(controller.getState().error?.code).toBe('read_failed');
    expect(controller.getState().game).toEqual(game);
  });

  describe('toggleModInActiveProfile', () => {
    async function withActive(modIds: string[]) {
      const ctx = await ready({ mods: [mod('a'), mod('b'), mod('c')] });
      await ctx.controller.createProfile('Mine');
      const id = ctx.controller.getState().profiles.find((p) => p.name === 'Mine')?.id ?? '';
      await ctx.controller.setProfileMods(id, modIds);
      await ctx.controller.setActiveProfile(id);
      ctx.controller.dismissNotice();
      return { ...ctx, id };
    }
    const modsOf = (c: { getState(): { profiles: Profile[] } }, id: string) =>
      c.getState().profiles.find((p) => p.id === id)?.mod_ids;

    it('When the mod is absent Then it is added at the end keeping order, with no notice', async () => {
      const { controller, id } = await withActive(['b', 'a']);
      await controller.toggleModInActiveProfile('c');
      expect(modsOf(controller, id)).toEqual(['b', 'a', 'c']);
      expect(controller.getState().notice).toBeNull();
      expect(controller.getState().error).toBeNull();
    });

    it('When the mod is present Then it is removed', async () => {
      const { controller, id } = await withActive(['b', 'a']);
      await controller.toggleModInActiveProfile('b');
      expect(modsOf(controller, id)).toEqual(['a']);
      expect(controller.getState().notice).toBeNull();
    });

    it('When there is no active profile Then nothing happens', async () => {
      const { controller, gateway } = await ready({
        mods: [mod('a')],
        config: { game_dir: game.root, active_profile: null },
      });
      await controller.toggleModInActiveProfile('a');
      expect(gateway.callsOf('setProfileMods')).toHaveLength(0);
    });

    it('When the active profile is Vanilla Then nothing happens', async () => {
      const { controller, gateway } = await ready({ mods: [mod('a')] });
      await controller.toggleModInActiveProfile('a');
      expect(gateway.callsOf('setProfileMods')).toHaveLength(0);
    });

    it.each(['starting', 'running'] as const)('When the session is %s Then nothing happens', async (kind) => {
      const { controller, gateway, session, id } = await withActive(['a']);
      await controller.play();
      if (kind === 'running') session.emitSession({ state: 'started', cleaned: 0, started_at: null });
      expect(controller.getState().session.kind).toBe(kind);
      const before = gateway.callsOf('setProfileMods').length;
      await controller.toggleModInActiveProfile('b');
      expect(gateway.callsOf('setProfileMods')).toHaveLength(before);
      expect(modsOf(controller, id)).toEqual(['a']);
    });

    it('When the gateway fails Then the error is exposed', async () => {
      const { controller, gateway } = await withActive(['a']);
      gateway.failNext('setProfileMods', { code: 'write_failed', message: 'x' });
      await controller.toggleModInActiveProfile('b');
      expect(controller.getState().error?.code).toBe('write_failed');
    });

    it('When the backend refuses because a session is active Then the error is exposed and mods are unchanged', async () => {
      const { controller, gateway, id } = await withActive(['a']);
      expect(controller.getState().session.kind).toBe('idle');
      gateway.failNext('setProfileMods', { code: 'profile_locked_by_session', message: 'locked' });
      await controller.toggleModInActiveProfile('b');
      expect(controller.getState().error?.code).toBe('profile_locked_by_session');
      expect(modsOf(controller, id)).toEqual(['a']);
    });
  });

  describe('session lock on library and profile writes', () => {
    async function locked() {
      const ctx = await ready({ mods: [mod('a'), mod('b')] });
      await ctx.controller.createProfile('Mine');
      const id = ctx.controller.getState().profiles.find((p) => p.name === 'Mine')?.id ?? '';
      ctx.controller.dismissNotice();
      ctx.session.emitSession({ state: 'started', cleaned: 0, started_at: null });
      expect(ctx.controller.getState().session.kind).toBe('running');
      const writes = ['createProfile', 'renameProfile', 'deleteProfile', 'setProfileMods', 'removeMod'] as const;
      const before = writes.map((method) => ctx.gateway.callsOf(method).length);
      return { ...ctx, id, writes, before };
    }

    it('When a session is running Then no library or profile write reaches the gateway', async () => {
      const { controller, gateway, id, writes, before } = await locked();
      await controller.createProfile('Wet');
      await controller.renameProfile(id, 'Dry');
      await controller.deleteProfile(id);
      await controller.setProfileMods(id, ['a']);
      await controller.removeMod('a');
      expect(writes.map((method) => gateway.callsOf(method).length)).toEqual(before);
      expect(controller.getState().mods).toHaveLength(2);
      expect(controller.getState().profiles.map((p) => p.name)).toEqual(['Vanilla', 'Mine']);
      expect(controller.getState().error).toBeNull();
    });

    it('When the session ends Then the writes work again', async () => {
      const { controller, session, gateway } = await locked();
      session.emitSession({ state: 'ended', cleaned: 0, started_at: null });
      await controller.removeMod('a');
      expect(gateway.callsOf('removeMod')).toHaveLength(1);
      expect(controller.getState().mods.map((m) => m.id)).toEqual(['b']);
    });
  });
});
