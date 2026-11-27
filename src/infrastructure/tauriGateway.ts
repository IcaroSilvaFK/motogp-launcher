import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';
import type { LauncherGateway } from '../application/ports';
import { toCommandError } from '../domain';
import type { CatalogInstallProgress, StorageCategory, StorageUsage } from '../domain';
import { subscribe } from './subscription';
import type { ListenFn } from './tauriSessionEvents';

export type InvokeFn = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

/** Wire shape of `get_storage_usage` (snake_case, as serialized by the backend). */
interface StorageUsageDto {
  root: string;
  total_bytes: number;
  free_bytes: number | null;
  categories: { category: StorageCategory; bytes: number }[];
}

function toStorageUsage(dto: StorageUsageDto): StorageUsage {
  return {
    root: dto.root,
    totalBytes: dto.total_bytes,
    freeBytes: dto.free_bytes,
    categories: dto.categories.map(({ category, bytes }) => ({ category, bytes })),
  };
}

export function createTauriGateway(invoke: InvokeFn = tauriInvoke, listen: ListenFn = tauriListen): LauncherGateway {
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
    listConflicts: (modIds) => call('list_conflicts', { modIds }),
    play: (profileId) => call('play', { profileId }),
    recoveryStatus: () => call('recovery_status'),
    currentSession: () => call('current_session'),
    stopSession: () => call('stop_session'),
    deploySession: () => call('deploy_session'),
    forgetStaleSession: () => call('forget_stale_session'),
    cleanStaleSession: (confirmedDir) => call('clean_stale_session', { confirmedDir }),
    restoreGameFiles: () => call('restore_game_files'),
    getStorageUsage: async () => toStorageUsage(await call<StorageUsageDto>('get_storage_usage')),
    getGameArtwork: () => call<string | null>('get_game_artwork'),
    openDataFolder: () => call('open_data_folder'),
    openSupportPage: () => call('open_support_page'),
    getCatalog: () => call('get_catalog'),
    refreshCatalog: () => call('refresh_catalog'),
    openCatalogPage: (id) => call('open_catalog_page', { id }),
    installCatalogEntry: (id, enableAfterInstall) => call('install_catalog_entry', { id, enableAfterInstall }),
    cancelCatalogInstall: (id) => call('cancel_catalog_install', { id }),
    onCatalogInstallProgress: (handler) =>
      subscribe<CatalogInstallProgress>(
        (deliver) => listen<CatalogInstallProgress>('catalog-install-progress', (event) => deliver(event.payload)),
        handler,
      ),
  };
}
