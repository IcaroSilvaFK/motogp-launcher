import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import type { PublishError, PublishRequest } from '../../domain';
import { createTauriModPublisher } from '../tauri/TauriModPublisher';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }));

const invokeMock = vi.mocked(invoke);

const rawVersion = {
  modId: 'brazil-livery',
  version: '1.0',
  title: 'Brazil Livery',
  description: 'D',
  category: 'livery',
  game: 'motogp25',
  fileName: 'b.zip',
  size: 10,
  sha256: 'ab',
  status: 'pending',
  screenshotCount: 0,
  uploadedAt: 'now',
};

const request: PublishRequest = {
  modId: 'brazil-livery',
  title: 'Brazil Livery',
  description: 'D',
  category: 'livery',
  game: 'motogp25',
  version: '1.0',
  file: 'C:\\m\\b.zip',
  screenshots: [],
  ownershipConfirmed: true,
};

beforeEach(() => invokeMock.mockReset());

async function rejection(promise: Promise<unknown>): Promise<PublishError> {
  try {
    await promise;
  } catch (error) {
    return error as PublishError;
  }
  throw new Error('expected a rejection');
}

describe('Given the Tauri mod publisher', () => {
  it('When publishing Then it calls community_publish_mod with the request and parses the version', async () => {
    invokeMock.mockResolvedValue(rawVersion);
    const version = await createTauriModPublisher(vi.fn()).publish(request);
    expect(invokeMock).toHaveBeenCalledWith('community_publish_mod', { request });
    expect(version.status).toBe('pending');
  });

  it('When the backend rejects Then the error is typed', async () => {
    invokeMock.mockRejectedValueOnce({ code: 'version_exists', message: 'exists' });
    const error = await rejection(createTauriModPublisher(vi.fn()).publish(request));
    expect(error.name).toBe('PublishError');
    expect(error.code).toBe('version_exists');
  });

  it('When the answer is malformed Then it fails with server_error', async () => {
    invokeMock.mockResolvedValue({ nope: true });
    const error = await rejection(createTauriModPublisher(vi.fn()).publish(request));
    expect(error.code).toBe('server_error');
  });

  it('When cancelling Then it calls community_cancel_publish', async () => {
    invokeMock.mockResolvedValue(null);
    await createTauriModPublisher(vi.fn()).cancel();
    expect(invokeMock).toHaveBeenCalledWith('community_cancel_publish', undefined);
  });

  it('When reading my mods Then it parses and a malformed answer is server_error', async () => {
    invokeMock.mockResolvedValueOnce([{ id: 'brazil-livery', versions: [rawVersion] }]);
    const mods = await createTauriModPublisher(vi.fn()).myMods();
    expect(invokeMock).toHaveBeenCalledWith('community_my_mods', undefined);
    expect(mods[0]?.versions).toHaveLength(1);
    invokeMock.mockResolvedValueOnce('nope');
    expect((await rejection(createTauriModPublisher(vi.fn()).myMods())).code).toBe('server_error');
  });

  it('When listening to progress Then valid events reach the callback until unsubscribed', async () => {
    const unlisten = vi.fn();
    let handler: ((event: { payload: unknown }) => void) | undefined;
    const listen = vi.fn((_event: string, h: (event: { payload: unknown }) => void) => {
      handler = h;
      return Promise.resolve(unlisten);
    });
    const callback = vi.fn();
    const stop = createTauriModPublisher(listen as never).onProgress(callback);
    expect(listen).toHaveBeenCalledWith('community-publish-progress', expect.any(Function));
    handler?.({ payload: { sent: 1, total: 2, phase: 'uploading' } });
    handler?.({ payload: { sent: 'x' } });
    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenCalledWith({ sent: 1, total: 2, phase: 'uploading' });
    stop();
    handler?.({ payload: { sent: 2, total: 2, phase: 'done' } });
    expect(callback).toHaveBeenCalledTimes(1);
    await Promise.resolve();
    await Promise.resolve();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('When the listener cannot be registered Then onProgress still returns a safe unsubscribe', () => {
    const listen = vi.fn(() => Promise.reject(new Error('no events')));
    const stop = createTauriModPublisher(listen as never).onProgress(vi.fn());
    expect(() => stop()).not.toThrow();
  });
});
