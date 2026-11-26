import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { CommunitySignInError } from '../../domain';
import { createTauriCommunityAccount } from '../tauri/TauriCommunityAccount';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

const signedIn = {
  state: 'signed_in',
  profile: { displayName: 'rider', avatarDataUrl: 'data:image/png;base64,AAAA' },
};

beforeEach(() => {
  invokeMock.mockReset();
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => vi.restoreAllMocks());

describe('createTauriCommunityAccount', () => {
  it('reads the status from community_me', async () => {
    invokeMock.mockResolvedValueOnce(signedIn);
    expect(await createTauriCommunityAccount().status()).toEqual(signedIn);
    expect(invokeMock).toHaveBeenCalledWith('community_me');
  });

  it('reports signed out and offline as they come', async () => {
    invokeMock.mockResolvedValueOnce({ state: 'signed_out' });
    expect(await createTauriCommunityAccount().status()).toEqual({ state: 'signed_out' });
    invokeMock.mockResolvedValueOnce({ state: 'offline' });
    expect(await createTauriCommunityAccount().status()).toEqual({ state: 'offline' });
  });

  it('treats a missing command or a rejection as offline without throwing', async () => {
    invokeMock.mockRejectedValueOnce(new Error('command community_me not found'));
    expect(await createTauriCommunityAccount().status()).toEqual({ state: 'offline' });
  });

  it('treats a synchronous throw as offline', async () => {
    invokeMock.mockImplementationOnce(() => {
      throw new Error('no ipc');
    });
    expect(await createTauriCommunityAccount().status()).toEqual({ state: 'offline' });
  });

  it('treats a malformed answer as offline', async () => {
    invokeMock.mockResolvedValueOnce({ state: 'signed_in' });
    expect(await createTauriCommunityAccount().status()).toEqual({ state: 'offline' });
    invokeMock.mockResolvedValueOnce(null);
    expect(await createTauriCommunityAccount().status()).toEqual({ state: 'offline' });
  });

  it('signs in through community_sign_in and returns the profile', async () => {
    invokeMock.mockResolvedValueOnce(signedIn);
    expect(await createTauriCommunityAccount().signIn()).toEqual(signedIn);
    expect(invokeMock).toHaveBeenCalledWith('community_sign_in');
  });

  it('rejects sign-in with a typed error for each backend code', async () => {
    const cases = {
      community_sign_in_failed: 'failed',
      community_sign_in_timeout: 'timeout',
      community_sign_in_denied: 'denied',
      community_offline: 'offline',
    };
    for (const [backend, code] of Object.entries(cases)) {
      invokeMock.mockRejectedValueOnce({ code: backend, message: 'x' });
      const error = await createTauriCommunityAccount()
        .signIn()
        .catch((e: unknown) => e);
      expect(error).toBeInstanceOf(CommunitySignInError);
      expect((error as CommunitySignInError).code).toBe(code);
    }
  });

  it('rejects sign-in with failed for an unknown rejection or a malformed answer', async () => {
    invokeMock.mockRejectedValueOnce('boom');
    await expect(createTauriCommunityAccount().signIn()).rejects.toMatchObject({ code: 'failed' });
    invokeMock.mockResolvedValueOnce({ state: 'signed_out' });
    await expect(createTauriCommunityAccount().signIn()).rejects.toMatchObject({ code: 'failed' });
    invokeMock.mockResolvedValueOnce('nope');
    await expect(createTauriCommunityAccount().signIn()).rejects.toBeInstanceOf(CommunitySignInError);
  });

  it('signs out through community_sign_out and never rejects', async () => {
    invokeMock.mockResolvedValueOnce(null);
    await createTauriCommunityAccount().signOut();
    expect(invokeMock).toHaveBeenCalledWith('community_sign_out');
    invokeMock.mockRejectedValueOnce(new Error('missing'));
    await expect(createTauriCommunityAccount().signOut()).resolves.toBeUndefined();
  });
});
