import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type { LauncherGateway } from '../application/ports';
import { toCommandError } from '../domain';

export type InvokeFn = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

export function createTauriGateway(invoke: InvokeFn = tauriInvoke): LauncherGateway {
  async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    try {
      return (await (args === undefined ? invoke(command) : invoke(command, args))) as T;
    } catch (error) {
      throw toCommandError(error);
    }
  }

  return {
    getConfig: () => call('get_config'),
    detectGames: () => call('detect_games'),
    setGameDir: (path) => call('set_game_dir', { path }),
    getGame: () => call('get_game'),
    listMods: () => call('list_mods'),
    importMods: (paths) => call('import_mods', { paths }),
    removeMod: (id) => call('remove_mod', { id }),
    listProfiles: () => call('list_profiles'),
    createProfile: (name) => call('create_profile', { name }),
    renameProfile: (id, name) => call('rename_profile', { id, name }),
    deleteProfile: (id) => call('delete_profile', { id }),
    setProfileMods: (id, modIds) => call('set_profile_mods', { id, modIds }),
    setActiveProfile: (id) => call('set_active_profile', { id }),
    play: (profileId) => call('play', { profileId }),
    recoveryStatus: () => call('recovery_status'),
    deploySession: () => call('deploy_session'),
    forgetStaleSession: () => call('forget_stale_session'),
    cleanStaleSession: (confirmedDir) => call('clean_stale_session', { confirmedDir }),
  };
}
