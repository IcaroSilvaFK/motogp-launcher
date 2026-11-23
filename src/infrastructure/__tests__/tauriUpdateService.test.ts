import { beforeEach, describe, expect, it, vi } from 'vitest';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import type { UpdateProgress } from '../../domain';
import { createTauriUpdateService } from '../tauri/TauriUpdateService';

vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));

type DownloadEvent =
  | { event: 'Started'; data: { contentLength?: number } }
  | { event: 'Progress'; data: { chunkLength: number } }
  | { event: 'Finished' };

function fakeUpdate(overrides: Record<string, unknown> = {}, events: DownloadEvent[] = [], downloadError?: unknown) {
  const downloadAndInstall = vi.fn(async (cb?: (e: DownloadEvent) => void) => {
    for (const e of events) cb?.(e);
    if (downloadError !== undefined) throw downloadError;
  });
  return {
    version: '1.1.0',
    currentVersion: '1.0.0',
    body: 'Notes',
    date: '2026-11-01T00:00:00Z',
    downloadAndInstall,
    ...overrides,
  };
}

const checkMock = vi.mocked(check);
const relaunchMock = vi.mocked(relaunch);

beforeEach(() => {
  vi.clearAllMocks();
});

describe('Given the Tauri update service', () => {
  it('When checking, downloading and restarting Then updater_adapter_checks_downloads_installs_and_restarts', async () => {
    const update = fakeUpdate({}, [
      { event: 'Started', data: { contentLength: 100 } },
      { event: 'Progress', data: { chunkLength: 40 } },
      { event: 'Finished' },
    ]);
    checkMock.mockResolvedValue(update as never);
    const service = createTauriUpdateService();

    expect(await service.check()).toEqual({
      version: '1.1.0',
      currentVersion: '1.0.0',
      notes: 'Notes',
      date: '2026-11-01T00:00:00Z',
    });
    const events: UpdateProgress[] = [];
    await service.downloadAndInstall((e) => events.push(e));
    expect(update.downloadAndInstall).toHaveBeenCalledTimes(1);
    expect(events).toEqual([{ kind: 'started', total: 100 }, { kind: 'progress', chunk: 40 }, { kind: 'finished' }]);
    await service.restart();
    expect(relaunchMock).toHaveBeenCalledTimes(1);
  });

  it('When the signature is invalid Then invalid_signature_error_is_propagated', async () => {
    checkMock.mockResolvedValue(fakeUpdate({}, [], 'signature verification failed') as never);
    const service = createTauriUpdateService();
    await service.check();
    await expect(service.downloadAndInstall(() => undefined)).rejects.toEqual({
      kind: 'signature',
      message: 'signature verification failed',
    });
  });

  it('When the plugin reports no update Then version_check_uses_updater_semantics', async () => {
    checkMock.mockResolvedValue(null as never);
    expect(await createTauriUpdateService().check()).toBeNull();
    // An update the plugin returns is passed through untouched, even if it looks older.
    checkMock.mockResolvedValue(fakeUpdate({ version: '0.9.0' }) as never);
    expect((await createTauriUpdateService().check())?.version).toBe('0.9.0');
  });

  it('When the check fails on the network Then it rejects with kind network', async () => {
    checkMock.mockRejectedValue(new Error('error sending request: connection refused'));
    await expect(createTauriUpdateService().check()).rejects.toEqual({
      kind: 'network',
      message: 'error sending request: connection refused',
    });
  });

  it('When the check fails for another reason Then it still rejects with kind network', async () => {
    checkMock.mockRejectedValue('boom');
    await expect(createTauriUpdateService().check()).rejects.toEqual({
      kind: 'network',
      message: 'boom',
    });
  });

  it('When the install fails Then it rejects with kind install', async () => {
    checkMock.mockResolvedValue(fakeUpdate({}, [], new Error('failed to run installer')) as never);
    const service = createTauriUpdateService();
    await service.check();
    await expect(service.downloadAndInstall(() => undefined)).rejects.toEqual({
      kind: 'install',
      message: 'failed to run installer',
    });
  });

  it('When the download fails on the network Then it rejects with kind network', async () => {
    checkMock.mockResolvedValue(fakeUpdate({}, [], 'request timeout') as never);
    const service = createTauriUpdateService();
    await service.check();
    await expect(service.downloadAndInstall(() => undefined)).rejects.toMatchObject({
      kind: 'network',
    });
  });

  it('When events are unknown or lack a content length Then they are mapped safely', async () => {
    const update = fakeUpdate({}, [{ event: 'Started', data: {} }, { event: 'Mystery' } as unknown as DownloadEvent]);
    checkMock.mockResolvedValue(update as never);
    const service = createTauriUpdateService();
    await service.check();
    const events: UpdateProgress[] = [];
    await service.downloadAndInstall((e) => events.push(e));
    expect(events).toEqual([{ kind: 'started', total: null }]);
  });

  it('When body and date are missing Then notes and date are null', async () => {
    checkMock.mockResolvedValue(fakeUpdate({ body: undefined, date: undefined }) as never);
    expect(await createTauriUpdateService().check()).toMatchObject({ notes: null, date: null });
  });

  it('When downloading without a prior available update Then it rejects', async () => {
    await expect(createTauriUpdateService().downloadAndInstall(() => undefined)).rejects.toMatchObject({
      kind: 'unknown',
    });
  });
});
