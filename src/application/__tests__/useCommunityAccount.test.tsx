import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { CommunitySignInError } from '../../domain';
import type { CommunityProfile, CommunityStatus } from '../../domain';
import type { CommunityAccount } from '../ports/CommunityAccount';
import { useCommunityAccount } from '../useCommunityAccount';

const profile: CommunityProfile = { displayName: 'rider', avatarDataUrl: null };
const signedIn: CommunityStatus = { state: 'signed_in', profile };

function account(overrides: Partial<CommunityAccount> = {}) {
  return {
    status: vi.fn(() => Promise.resolve<CommunityStatus>({ state: 'signed_out' })),
    signIn: vi.fn(() => Promise.resolve(signedIn)),
    signOut: vi.fn(() => Promise.resolve()),
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('useCommunityAccount', () => {
  it('is unavailable and requests nothing without a port', () => {
    const { result } = renderHook(() => useCommunityAccount(undefined));
    expect(result.current.available).toBe(false);
    expect(result.current.state).toBe('signedOut');
    act(() => result.current.signIn());
    expect(result.current.state).toBe('signedOut');
  });

  it('loads the status on mount', async () => {
    const port = account({ status: vi.fn(() => Promise.resolve(signedIn)) });
    const { result } = renderHook(() => useCommunityAccount(port));
    expect(result.current.loaded).toBe(false);
    await waitFor(() => expect(result.current.loaded).toBe(true));
    expect(result.current).toMatchObject({ available: true, state: 'signedIn', profile, errorCode: null });
  });

  it('shows offline and signed out statuses', async () => {
    const offline = account({ status: vi.fn(() => Promise.resolve<CommunityStatus>({ state: 'offline' })) });
    const first = renderHook(() => useCommunityAccount(offline));
    await waitFor(() => expect(first.result.current.state).toBe('offline'));
    const idle = account();
    const out = renderHook(() => useCommunityAccount(idle));
    await waitFor(() => expect(out.result.current.loaded).toBe(true));
    expect(out.result.current.state).toBe('signedOut');
  });

  it('falls back to offline when the port misbehaves', async () => {
    const rejecting = account({ status: vi.fn(() => Promise.reject(new Error('x'))) });
    const first = renderHook(() => useCommunityAccount(rejecting));
    await waitFor(() => expect(first.result.current.state).toBe('offline'));
    const throwing = account({
      status: vi.fn(() => {
        throw new Error('sync');
      }),
    });
    const second = renderHook(() => useCommunityAccount(throwing));
    await waitFor(() => expect(second.result.current.state).toBe('offline'));
  });

  it('signs in: signingIn while waiting, then signedIn with the profile', async () => {
    const pending = deferred<CommunityStatus>();
    const port = account({ signIn: vi.fn(() => pending.promise) });
    const { result } = renderHook(() => useCommunityAccount(port));
    await waitFor(() => expect(result.current.loaded).toBe(true));
    act(() => result.current.signIn());
    expect(result.current.state).toBe('signingIn');
    await act(async () => pending.resolve(signedIn));
    expect(result.current).toMatchObject({ state: 'signedIn', profile, errorCode: null });
  });

  it('ignores a second sign-in while one is running', async () => {
    const pending = deferred<CommunityStatus>();
    const port = account({ signIn: vi.fn(() => pending.promise) });
    const { result } = renderHook(() => useCommunityAccount(port));
    await waitFor(() => expect(result.current.loaded).toBe(true));
    act(() => result.current.signIn());
    act(() => result.current.signIn());
    expect(port.signIn).toHaveBeenCalledTimes(1);
    await act(async () => pending.resolve(signedIn));
  });

  it('keeps the error code and goes back to signed out when sign-in fails', async () => {
    for (const code of ['failed', 'timeout', 'denied'] as const) {
      const port = account({ signIn: vi.fn(() => Promise.reject(new CommunitySignInError(code))) });
      const { result } = renderHook(() => useCommunityAccount(port));
      await waitFor(() => expect(result.current.loaded).toBe(true));
      await act(async () => result.current.signIn());
      expect(result.current).toMatchObject({ state: 'signedOut', errorCode: code });
    }
  });

  it('goes offline when the server cannot be reached during sign-in', async () => {
    const port = account({ signIn: vi.fn(() => Promise.reject(new CommunitySignInError('offline'))) });
    const { result } = renderHook(() => useCommunityAccount(port));
    await waitFor(() => expect(result.current.loaded).toBe(true));
    await act(async () => result.current.signIn());
    expect(result.current).toMatchObject({ state: 'offline', errorCode: 'offline' });
  });

  it('treats an untyped rejection or a synchronous throw as failed', async () => {
    const rejecting = account({ signIn: vi.fn(() => Promise.reject(new Error('boom'))) });
    const first = renderHook(() => useCommunityAccount(rejecting));
    await waitFor(() => expect(first.result.current.loaded).toBe(true));
    await act(async () => first.result.current.signIn());
    expect(first.result.current.errorCode).toBe('failed');
    const throwing = account({
      signIn: vi.fn(() => {
        throw new Error('sync');
      }),
    });
    const second = renderHook(() => useCommunityAccount(throwing));
    await waitFor(() => expect(second.result.current.loaded).toBe(true));
    await act(async () => second.result.current.signIn());
    expect(second.result.current.errorCode).toBe('failed');
  });

  it('clears the previous error when a new sign-in starts', async () => {
    const signIn = vi
      .fn<CommunityAccount['signIn']>()
      .mockRejectedValueOnce(new CommunitySignInError('timeout'))
      .mockResolvedValueOnce(signedIn);
    const port = account({ signIn });
    const { result } = renderHook(() => useCommunityAccount(port));
    await waitFor(() => expect(result.current.loaded).toBe(true));
    await act(async () => result.current.signIn());
    expect(result.current.errorCode).toBe('timeout');
    await act(async () => result.current.signIn());
    expect(result.current).toMatchObject({ state: 'signedIn', errorCode: null });
  });

  it('signs out and clears the profile, even if the port rejects', async () => {
    const port = account({ status: vi.fn(() => Promise.resolve(signedIn)) });
    const { result } = renderHook(() => useCommunityAccount(port));
    await waitFor(() => expect(result.current.state).toBe('signedIn'));
    await act(async () => result.current.signOut());
    expect(port.signOut).toHaveBeenCalledTimes(1);
    expect(result.current).toMatchObject({ state: 'signedOut', profile: null });

    const failing = account({
      status: vi.fn(() => Promise.resolve(signedIn)),
      signOut: vi.fn(() => Promise.reject(new Error('x'))),
    });
    const second = renderHook(() => useCommunityAccount(failing));
    await waitFor(() => expect(second.result.current.state).toBe('signedIn'));
    await act(async () => second.result.current.signOut());
    expect(second.result.current.state).toBe('signedOut');

    const throwing = account({
      status: vi.fn(() => Promise.resolve(signedIn)),
      signOut: vi.fn(() => {
        throw new Error('sync');
      }),
    });
    const third = renderHook(() => useCommunityAccount(throwing));
    await waitFor(() => expect(third.result.current.state).toBe('signedIn'));
    act(() => third.result.current.signOut());
    expect(third.result.current.state).toBe('signedOut');
  });

  it('does not update after unmount', async () => {
    const pending = deferred<CommunityStatus>();
    const port = account({ signIn: vi.fn(() => pending.promise) });
    const { result, unmount } = renderHook(() => useCommunityAccount(port));
    await waitFor(() => expect(result.current.loaded).toBe(true));
    act(() => result.current.signIn());
    unmount();
    await act(async () => pending.resolve(signedIn));
    expect(result.current.state).toBe('signingIn');
  });

  it('does not let a late status answer undo a sign-in in flight', async () => {
    const late = deferred<CommunityStatus>();
    const pending = deferred<CommunityStatus>();
    const port = account({ status: vi.fn(() => late.promise), signIn: vi.fn(() => pending.promise) });
    const { result } = renderHook(() => useCommunityAccount(port));
    act(() => result.current.signIn());
    await act(async () => late.resolve({ state: 'signed_out' }));
    expect(result.current.state).toBe('signingIn');
    await act(async () => pending.resolve(signedIn));
    expect(result.current.state).toBe('signedIn');
  });
});
