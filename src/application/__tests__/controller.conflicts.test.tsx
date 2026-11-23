import { describe, expect, it } from 'vitest';
import type { FileConflict, GameInfo, ModEntry, Profile } from '../../domain';
import { VANILLA_ID } from '../../domain';
import { createLauncherController } from '../createLauncherController';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from './fakes';
import type { FakeGatewayOptions } from './fakes';

const game: GameInfo = { name: 'MotoGP25', root: 'C:/g', mods_dir: 'C:/g/~mods', app_id: 1234 };
const mod = (id: string): ModEntry => ({ id, name: id, files: [`${id}.pak`], size_bytes: 1 });

function setup(options: FakeGatewayOptions) {
  const profiles = options.profiles?.map((p) => ({ ...p, mod_ids: [...p.mod_ids] }));
  const gateway = new FakeGateway({ game, ...options, profiles });
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session: new FakeSessionEvents({}),
    drop: new FakeDropEvents({}),
  });
  return { gateway, controller };
}

async function ready(options: FakeGatewayOptions) {
  const ctx = setup(options);
  await ctx.controller.start();
  return ctx;
}

const conflict: FileConflict = { path: 'Ducati_P.pak', mod_ids: ['a', 'b'], winner: 'b' };
const conflictsFor = (modIds: string[]): FileConflict[] => (modIds.length > 1 ? [conflict] : []);
const profileP: Profile = { id: 'p', name: 'P', mod_ids: ['a', 'b', 'c'] };
const profileQ: Profile = { id: 'q', name: 'Q', mod_ids: ['a'] };
const options: FakeGatewayOptions = {
  mods: [mod('a'), mod('b'), mod('c')],
  profiles: [{ id: VANILLA_ID, name: 'Vanilla', mod_ids: [] }, profileP, profileQ],
  config: { game_dir: game.root, active_profile: 'p' },
  conflictsFor,
};

describe('Given the load order conflicts of the active profile', () => {
  it('When started Then conflicts_of_active_profile_are_loaded', async () => {
    const { controller, gateway } = await ready(options);
    expect(controller.getState().conflicts).toEqual([conflict]);
    expect(gateway.callsOf('listConflicts').map((c) => c.args)).toEqual([[['a', 'b', 'c']]]);
  });

  it('When there is no active profile Then conflicts_are_empty_without_calling_the_gateway', async () => {
    const { controller, gateway } = await ready({
      ...options,
      config: { game_dir: game.root, active_profile: null },
    });
    expect(controller.getState().conflicts).toEqual([]);
    expect(gateway.callsOf('listConflicts')).toHaveLength(0);
  });

  it('When the active profile has no mods Then conflicts_are_empty_without_calling_the_gateway', async () => {
    const { controller, gateway } = await ready({
      ...options,
      config: { game_dir: game.root, active_profile: VANILLA_ID },
    });
    expect(controller.getState().conflicts).toEqual([]);
    expect(gateway.callsOf('listConflicts')).toHaveLength(0);
  });

  it('When the active profile changes Then conflicts_are_refreshed', async () => {
    const { controller, gateway } = await ready(options);
    await controller.setActiveProfile('q');
    expect(controller.getState().conflicts).toEqual([]);
    await controller.setActiveProfile('p');
    expect(controller.getState().conflicts).toEqual([conflict]);
    expect(gateway.callsOf('listConflicts')).toHaveLength(3);
  });

  it('When the profile mods are saved Then conflicts_follow_the_new_order', async () => {
    const { controller, gateway } = await ready(options);
    await controller.setProfileMods('p', ['c', 'b', 'a']);
    expect(gateway.callsOf('listConflicts').at(-1)?.args).toEqual([['c', 'b', 'a']]);
    expect(controller.getState().notice).toEqual({ kind: 'profile-saved' });
  });

  it('When mods are imported Then conflicts_are_refreshed', async () => {
    const { controller, gateway } = await ready(options);
    await controller.importPaths(['C:/x/d.pak']);
    expect(gateway.callsOf('listConflicts')).toHaveLength(2);
  });

  it('When a mod is removed Then conflicts_are_refreshed_without_it', async () => {
    const { controller, gateway } = await ready(options);
    await controller.removeMod('c');
    expect(gateway.callsOf('listConflicts').at(-1)?.args).toEqual([['a', 'b']]);
  });

  it('When a profile is deleted Then conflicts_are_refreshed', async () => {
    const { controller, gateway } = await ready(options);
    await controller.deleteProfile('q');
    expect(gateway.callsOf('listConflicts')).toHaveLength(2);
  });

  it('When the refresh fails Then the action_succeeds_conflicts_clear_and_the_error_is_exposed', async () => {
    const { controller, gateway } = await ready(options);
    gateway.failNext('listConflicts', { code: 'read_failed', message: 'x' });
    await controller.setProfileMods('p', ['b', 'a']);
    const state = controller.getState();
    expect(state.profiles.find((p) => p.id === 'p')?.mod_ids).toEqual(['b', 'a']);
    expect(state.conflicts).toEqual([]);
    expect(state.error?.code).toBe('read_failed');
    expect(state.notice).toBeNull();
    expect(state.busy).toBe(false);
  });

  it('When the initial refresh fails Then conflicts_are_empty_and_the_error_is_exposed', async () => {
    const { controller, gateway } = setup(options);
    gateway.failNext('listConflicts', { code: 'read_failed', message: 'x' });
    await controller.start();
    expect(controller.getState().conflicts).toEqual([]);
    expect(controller.getState().error?.code).toBe('read_failed');
    expect(controller.getState().status).toBe('ready');
  });

  it('When previewing a draft order Then it_returns_conflicts_without_changing_state', async () => {
    const { controller, gateway } = await ready(options);
    const before = controller.getState();
    expect(await controller.previewConflicts(['x', 'y'])).toEqual([conflict]);
    expect(gateway.callsOf('listConflicts').at(-1)?.args).toEqual([['x', 'y']]);
    expect(controller.getState()).toBe(before);
  });

  it('When previewing an empty draft Then it_resolves_empty_without_calling_the_gateway', async () => {
    const { controller, gateway } = await ready(options);
    const calls = gateway.callsOf('listConflicts').length;
    expect(await controller.previewConflicts([])).toEqual([]);
    expect(gateway.callsOf('listConflicts')).toHaveLength(calls);
  });

  it('When the preview fails Then it_resolves_empty_and_sets_the_error', async () => {
    const { controller, gateway } = await ready(options);
    gateway.failNext('listConflicts', { code: 'read_failed', message: 'x' });
    expect(await controller.previewConflicts(['x', 'y'])).toEqual([]);
    expect(controller.getState().error?.code).toBe('read_failed');
    expect(controller.getState().conflicts).toEqual([conflict]);
  });
});
