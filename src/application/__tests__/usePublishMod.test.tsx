import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { PublishError } from '../../domain';
import type { ModVersion, MyMod, PublishForm, PublishProgress } from '../../domain';
import type { ModPublisher } from '../ports/ModPublisher';
import { useMyMods } from '../useMyMods';
import { usePublishMod } from '../usePublishMod';

const version: ModVersion = {
  modId: 'brazil-livery',
  version: '1.0',
  title: 'Brazil Livery',
  description: 'D',
  category: 'livery',
  game: 'motogp25',
  fileName: 'b.zip',
  size: 1,
  sha256: 'ab',
  status: 'pending',
  screenshotCount: 0,
  uploadedAt: 'now',
};

const form: PublishForm = {
  modId: 'brazil-livery',
  title: 'Brazil Livery',
  description: 'D',
  longDescription: '',
  category: 'livery',
  game: 'motogp25',
  version: '1.0',
  file: 'C:\\m\\b.zip',
  screenshots: [],
  ownershipConfirmed: true,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function fake(overrides: Partial<ModPublisher> = {}) {
  let listener: ((p: PublishProgress) => void) | null = null;
  const unsubscribe = vi.fn(() => {
    listener = null;
  });
  const port = {
    publish: vi.fn(() => Promise.resolve(version)),
    cancel: vi.fn(() => Promise.resolve()),
    myMods: vi.fn((): Promise<MyMod[]> => Promise.resolve([])),
    onProgress: vi.fn((cb: (p: PublishProgress) => void) => {
      listener = cb;
      return unsubscribe;
    }),
    ...overrides,
  };
  return { port, unsubscribe, emit: (p: PublishProgress) => listener?.(p) };
}

describe('usePublishMod', () => {
  it('is unavailable without a port and starts idle', () => {
    expect(renderHook(() => usePublishMod(undefined)).result.current.available).toBe(false);
    const { port } = fake();
    const { result } = renderHook(() => usePublishMod(port));
    expect(result.current.available).toBe(true);
    expect(result.current.state).toEqual({ status: 'idle' });
  });

  it('refuses an invalid form without calling the port and exposes the field errors', async () => {
    const { port } = fake();
    const { result } = renderHook(() => usePublishMod(port));
    let started = true;
    await act(async () => {
      started = await result.current.start({ ...form, title: '', ownershipConfirmed: false });
    });
    expect(started).toBe(false);
    expect(port.publish).not.toHaveBeenCalled();
    expect(result.current.state).toEqual({ status: 'idle' });
    expect(result.current.errors).toEqual({ title: 'titleRequired', ownershipConfirmed: 'ownershipRequired' });
  });

  it('publishes: progress is tracked and the stored version is the result', async () => {
    const run = deferred<ModVersion>();
    const { port, emit, unsubscribe } = fake({ publish: vi.fn(() => run.promise) });
    const { result } = renderHook(() => usePublishMod(port));
    let done: Promise<boolean> = Promise.resolve(false);
    act(() => {
      done = result.current.start(form);
    });
    expect(result.current.state).toEqual({ status: 'publishing', progress: null });
    expect(port.publish).toHaveBeenCalledWith(
      expect.objectContaining({ modId: 'brazil-livery', file: 'C:\\m\\b.zip' }),
    );
    act(() => emit({ sent: 5, total: 10, phase: 'uploading' }));
    expect(result.current.state).toEqual({
      status: 'publishing',
      progress: { sent: 5, total: 10, phase: 'uploading' },
    });
    await act(async () => {
      run.resolve(version);
      expect(await done).toBe(true);
    });
    expect(result.current.state).toEqual({ status: 'done', version });
    expect(unsubscribe).toHaveBeenCalled();
  });

  it('ignores a second start while one is running', async () => {
    const run = deferred<ModVersion>();
    const { port } = fake({ publish: vi.fn(() => run.promise) });
    const { result } = renderHook(() => usePublishMod(port));
    act(() => {
      void result.current.start(form);
    });
    let second = true;
    await act(async () => {
      second = await result.current.start(form);
    });
    expect(second).toBe(false);
    expect(port.publish).toHaveBeenCalledTimes(1);
    run.resolve(version);
  });

  it('fails with the typed code and message', async () => {
    const { port } = fake({
      publish: vi.fn(() => Promise.reject(new PublishError('file_too_large', 'too big'))),
    });
    const { result } = renderHook(() => usePublishMod(port));
    await act(async () => {
      expect(await result.current.start(form)).toBe(false);
    });
    expect(result.current.state).toEqual({ status: 'failed', code: 'file_too_large', message: 'too big' });
  });

  it('maps an unknown rejection to server_error', async () => {
    const { port } = fake({ publish: vi.fn(() => Promise.reject('boom')) });
    const { result } = renderHook(() => usePublishMod(port));
    await act(async () => {
      await result.current.start(form);
    });
    expect(result.current.state).toMatchObject({ status: 'failed', code: 'server_error' });
  });

  it('cancel asks the port and a cancelled rejection ends in the cancelled state', async () => {
    const run = deferred<ModVersion>();
    const { port } = fake({ publish: vi.fn(() => run.promise) });
    const { result } = renderHook(() => usePublishMod(port));
    act(() => {
      void result.current.start(form);
    });
    act(() => result.current.cancel());
    expect(port.cancel).toHaveBeenCalled();
    await act(async () => {
      run.reject(new PublishError('cancelled'));
    });
    await waitFor(() => expect(result.current.state).toEqual({ status: 'cancelled' }));
  });

  it('cancel does nothing when no publish runs', () => {
    const { port } = fake();
    const { result } = renderHook(() => usePublishMod(port));
    act(() => result.current.cancel());
    expect(port.cancel).not.toHaveBeenCalled();
  });

  it('reset returns to idle after a finished attempt, but not while publishing', async () => {
    const { port } = fake({ publish: vi.fn(() => Promise.reject(new PublishError('offline'))) });
    const { result } = renderHook(() => usePublishMod(port));
    await act(async () => {
      await result.current.start(form);
    });
    act(() => result.current.reset());
    expect(result.current.state).toEqual({ status: 'idle' });
    expect(result.current.errors).toEqual({});
  });

  it('drops late answers after unmount and stops listening', async () => {
    const run = deferred<ModVersion>();
    const { port, unsubscribe } = fake({ publish: vi.fn(() => run.promise) });
    const { result, unmount } = renderHook(() => usePublishMod(port));
    act(() => {
      void result.current.start(form);
    });
    unmount();
    expect(unsubscribe).toHaveBeenCalled();
    run.resolve(version);
  });
});

describe('useMyMods', () => {
  const mods: MyMod[] = [{ id: 'brazil-livery', versions: [version] }];

  it('is unavailable without a port and requests nothing', () => {
    const { result } = renderHook(() => useMyMods(undefined));
    expect(result.current.available).toBe(false);
  });

  it('loads: loading then ready', async () => {
    const { port } = fake({ myMods: vi.fn(() => Promise.resolve(mods)) });
    const { result } = renderHook(() => useMyMods(port));
    expect(result.current.status).toBe('loading');
    await waitFor(() => expect(result.current.status).toBe('ready'));
    expect(result.current.mods).toEqual(mods);
    expect(result.current.error).toBeNull();
  });

  it('shows an error with the code and refresh retries', async () => {
    const myMods = vi.fn().mockRejectedValueOnce(new PublishError('offline')).mockResolvedValueOnce(mods);
    const { port } = fake({ myMods });
    const { result } = renderHook(() => useMyMods(port));
    await waitFor(() => expect(result.current.status).toBe('error'));
    expect(result.current.error?.code).toBe('offline');
    act(() => result.current.refresh());
    await waitFor(() => expect(result.current.status).toBe('ready'));
    expect(result.current.mods).toEqual(mods);
  });

  it('keeps the previous list while refreshing', async () => {
    const next = deferred<MyMod[]>();
    const myMods = vi.fn().mockResolvedValueOnce(mods).mockReturnValueOnce(next.promise);
    const { port } = fake({ myMods });
    const { result } = renderHook(() => useMyMods(port));
    await waitFor(() => expect(result.current.status).toBe('ready'));
    act(() => result.current.refresh());
    expect(result.current.refreshing).toBe(true);
    expect(result.current.mods).toEqual(mods);
    await act(async () => next.resolve([]));
    expect(result.current.refreshing).toBe(false);
    expect(result.current.mods).toEqual([]);
  });
});
