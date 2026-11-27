import { describe, expect, it, vi } from 'vitest';
import { createTauriGateway } from '../tauriGateway';
import type { LauncherGateway } from '../../application/ports';

const CASES: [keyof LauncherGateway, unknown[], string, Record<string, unknown> | undefined][] = [
  ['getConfig', [], 'get_config', undefined],
  ['detectGames', [], 'detect_games', undefined],
  ['setGameDir', ['C:/g'], 'set_game_dir', { path: 'C:/g' }],
  ['getGame', [], 'get_game', undefined],
  ['listMods', [], 'list_mods', undefined],
  ['importMods', [['a.pak']], 'import_mods', { paths: ['a.pak'] }],
  ['removeMod', ['m1'], 'remove_mod', { id: 'm1' }],
  ['listProfiles', [], 'list_profiles', undefined],
  ['createProfile', ['Race'], 'create_profile', { name: 'Race' }],
  ['renameProfile', ['p1', 'New'], 'rename_profile', { id: 'p1', name: 'New' }],
  ['deleteProfile', ['p1'], 'delete_profile', { id: 'p1' }],
  ['setProfileMods', ['p1', ['m1', 'm2']], 'set_profile_mods', { id: 'p1', modIds: ['m1', 'm2'] }],
  ['setActiveProfile', ['p1'], 'set_active_profile', { id: 'p1' }],
  ['listConflicts', [['m1', 'm2']], 'list_conflicts', { modIds: ['m1', 'm2'] }],
  ['play', ['p1'], 'play', { profileId: 'p1' }],
  ['recoveryStatus', [], 'recovery_status', undefined],
  ['currentSession', [], 'current_session', undefined],
  ['stopSession', [], 'stop_session', undefined],
  ['deploySession', [], 'deploy_session', undefined],
  ['forgetStaleSession', [], 'forget_stale_session', undefined],
  ['cleanStaleSession', ['D:/old/~mods'], 'clean_stale_session', { confirmedDir: 'D:/old/~mods' }],
  ['restoreGameFiles', [], 'restore_game_files', undefined],
  ['openDataFolder', [], 'open_data_folder', undefined],
  ['openSupportPage', [], 'open_support_page', undefined],
];

describe('Given the Tauri gateway', () => {
  it.each(CASES)(
    'When %s is called Then invoke_uses_expected_commands_and_argument_names',
    async (method, args, command, payload) => {
      const invoke = vi.fn().mockResolvedValue('result');
      const gateway = createTauriGateway(invoke);
      const result = await (gateway[method] as (...a: unknown[]) => Promise<unknown>)(...args);
      expect(result).toBe('result');
      expect(invoke).toHaveBeenCalledTimes(1);
      if (payload === undefined) expect(invoke.mock.calls[0]).toEqual([command]);
      else expect(invoke).toHaveBeenCalledWith(command, payload);
    },
  );

  it('When a command rejects with a backend error Then errors_are_normalized', async () => {
    const invoke = vi.fn().mockRejectedValue({ code: 'not_unreal_game', message: 'nope' });
    await expect(createTauriGateway(invoke).getGame()).rejects.toEqual({
      code: 'not_unreal_game',
      message: 'nope',
    });
  });

  it('When a command rejects with a plain string Then errors_are_normalized to unknown', async () => {
    const invoke = vi.fn().mockRejectedValue('boom');
    await expect(createTauriGateway(invoke).play('p1')).rejects.toEqual({
      code: 'unknown',
      message: 'boom',
    });
  });

  it('When the storage usage is requested Then get_storage_usage is invoked and the DTO is mapped to camelCase', async () => {
    // An accented Windows profile folder on purpose: real users have them and the path must pass through as is.
    const root = 'C:\\Users\\Usuário Padrão\\AppData\\Roaming\\motogp-launcher';
    const categories = [
      { category: 'mods', bytes: 1000 },
      { category: 'launcher_data', bytes: 30 },
      { category: 'other', bytes: 5 },
    ];
    const invoke = vi.fn().mockResolvedValue({ root, total_bytes: 1035, free_bytes: null, categories });
    await expect(createTauriGateway(invoke).getStorageUsage()).resolves.toEqual({
      root,
      totalBytes: 1035,
      freeBytes: null,
      categories,
    });
    expect(invoke.mock.calls[0]).toEqual(['get_storage_usage']);
  });

  it('When the backend reports the free space Then it is kept as a number', async () => {
    const invoke = vi.fn().mockResolvedValue({ root: 'C:/d', total_bytes: 0, free_bytes: 4096, categories: [] });
    await expect(createTauriGateway(invoke).getStorageUsage()).resolves.toMatchObject({ freeBytes: 4096 });
  });

  it('When get_storage_usage rejects Then the error is normalized', async () => {
    const invoke = vi.fn().mockRejectedValue({ code: 'read_failed', message: 'denied' });
    await expect(createTauriGateway(invoke).getStorageUsage()).rejects.toEqual({
      code: 'read_failed',
      message: 'denied',
    });
  });
});

describe('Given the game artwork command', () => {
  it('When get_game_artwork resolves a data URL Then it is returned as is', async () => {
    const invoke = vi.fn().mockResolvedValue('data:image/png;base64,AQID');
    await expect(createTauriGateway(invoke).getGameArtwork()).resolves.toBe('data:image/png;base64,AQID');
    expect(invoke.mock.calls[0]).toEqual(['get_game_artwork']);
  });

  it('When there is no artwork Then it resolves null', async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    await expect(createTauriGateway(invoke).getGameArtwork()).resolves.toBeNull();
  });
});

describe('Given the catalog install commands', () => {
  it('When installing and cancelling Then the commands take the id and the auto-enable flag', async () => {
    const invoke = vi.fn().mockResolvedValue([]);
    const gateway = createTauriGateway(invoke);
    await gateway.installCatalogEntry('rebalanced-ai', true);
    await gateway.cancelCatalogInstall('rebalanced-ai');
    expect(invoke).toHaveBeenNthCalledWith(1, 'install_catalog_entry', {
      id: 'rebalanced-ai',
      enableAfterInstall: true,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, 'cancel_catalog_install', { id: 'rebalanced-ai' });
  });

  it('When the backend rejects Then the code is preserved', async () => {
    const invoke = vi.fn().mockRejectedValue({ code: 'download_hash_mismatch', message: 'x' });
    await expect(createTauriGateway(invoke).installCatalogEntry('a', false)).rejects.toEqual({
      code: 'download_hash_mismatch',
      message: 'x',
    });
  });

  it('When subscribed to progress Then the event payload is delivered until unsubscribed', async () => {
    const unlisten = vi.fn();
    let handler: (event: { payload: unknown }) => void = () => undefined;
    const listen = vi.fn((_name: string, h: typeof handler) => {
      handler = h;
      return Promise.resolve(unlisten);
    });
    const received = vi.fn();
    const off = createTauriGateway(vi.fn(), listen as never).onCatalogInstallProgress(received);
    const payload = { id: 'a', phase: 'downloading', received: 1, total: 2 };
    handler({ payload });
    expect(listen.mock.calls[0]?.[0]).toBe('catalog-install-progress');
    expect(received).toHaveBeenCalledWith(payload);
    off();
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledTimes(1);
    handler({ payload });
    expect(received).toHaveBeenCalledTimes(1);
  });
});
