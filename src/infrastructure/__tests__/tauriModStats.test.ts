import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { ModStatsError } from '../../domain';
import { createTauriModStats } from '../tauri/TauriModStats';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

beforeEach(() => {
  invokeMock.mockReset();
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => vi.restoreAllMocks());

describe('createTauriModStats.stats', () => {
  it('reads the list from community_mod_stats', async () => {
    invokeMock.mockResolvedValueOnce({
      mods: [
        { id: 'a-mod', stars: 3, starredByMe: true },
        { id: 'b', stars: 1 },
      ],
    });
    expect(await createTauriModStats().stats()).toEqual([
      { id: 'a-mod', stars: 3, starredByMe: true },
      { id: 'b', stars: 1 },
    ]);
    expect(invokeMock).toHaveBeenCalledWith('community_mod_stats');
  });

  it('drops malformed items and keeps the valid ones', async () => {
    invokeMock.mockResolvedValueOnce({
      mods: [{ id: 'a', stars: 2 }, { id: '', stars: 1 }, { id: 'x', stars: -1 }, null],
    });
    expect(await createTauriModStats().stats()).toEqual([{ id: 'a', stars: 2 }]);
  });

  it('is empty, without throwing, on a rejection or a malformed answer', async () => {
    invokeMock.mockRejectedValueOnce({ code: 'community_offline', message: 'x' });
    expect(await createTauriModStats().stats()).toEqual([]);
    invokeMock.mockResolvedValueOnce({ nope: true });
    expect(await createTauriModStats().stats()).toEqual([]);
    invokeMock.mockResolvedValueOnce(null);
    expect(await createTauriModStats().stats()).toEqual([]);
  });
});

describe('createTauriModStats.star / unstar', () => {
  it('stars through community_star and returns the new numbers', async () => {
    invokeMock.mockResolvedValueOnce({ id: 'a-mod', stars: 11, starredByMe: true });
    expect(await createTauriModStats().star('a-mod')).toEqual({ id: 'a-mod', stars: 11, starredByMe: true });
    expect(invokeMock).toHaveBeenCalledWith('community_star', { id: 'a-mod' });
  });

  it('unstars through community_unstar', async () => {
    invokeMock.mockResolvedValueOnce({ id: 'a-mod', stars: 10, starredByMe: false });
    expect(await createTauriModStats().unstar('a-mod')).toEqual({ id: 'a-mod', stars: 10, starredByMe: false });
    expect(invokeMock).toHaveBeenCalledWith('community_unstar', { id: 'a-mod' });
  });

  it.each([
    ['community_signed_out', 'signedOut'],
    ['community_offline', 'offline'],
    ['community_request_failed', 'failed'],
    ['something_else', 'failed'],
  ])('maps the backend code %s to %s', async (code, kind) => {
    invokeMock.mockRejectedValueOnce({ code, message: 'x' });
    await expect(createTauriModStats().star('a')).rejects.toMatchObject({ name: 'ModStatsError', kind });
  });

  it('rejects with failed on an unknown rejection or a malformed answer', async () => {
    invokeMock.mockRejectedValueOnce(new Error('boom'));
    await expect(createTauriModStats().unstar('a')).rejects.toBeInstanceOf(ModStatsError);
    invokeMock.mockResolvedValueOnce({ id: 'a' });
    await expect(createTauriModStats().star('a')).rejects.toMatchObject({ kind: 'failed' });
  });
});
