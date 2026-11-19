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
  ['play', ['p1'], 'play', { profileId: 'p1' }],
  ['recoveryStatus', [], 'recovery_status', undefined],
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
});
