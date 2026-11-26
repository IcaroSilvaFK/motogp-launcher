import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { createTauriDownloadStats } from '../tauri/TauriDownloadStats';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

const series = Array.from({ length: 30 }, (_, index) => ({
  date: new Date(Date.UTC(2026, 9, 28 + index)).toISOString().slice(0, 10),
  count: index,
}));

beforeEach(() => {
  invokeMock.mockReset();
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => vi.restoreAllMocks());

describe('createTauriDownloadStats', () => {
  it('reports a download with the mod id only', async () => {
    invokeMock.mockResolvedValueOnce(null);
    await createTauriDownloadStats().report('rebalanced-ai');
    expect(invokeMock).toHaveBeenCalledWith('report_mod_download', { id: 'rebalanced-ai' });
  });

  it('never rejects when the report fails', async () => {
    invokeMock.mockRejectedValueOnce(new Error('command not found'));
    await expect(createTauriDownloadStats().report('a')).resolves.toBeUndefined();
  });

  it('reads and validates the totals', async () => {
    invokeMock.mockResolvedValueOnce([{ id: 'a', total: 3, last30Days: 1 }]);
    expect(await createTauriDownloadStats().totals()).toEqual([{ id: 'a', total: 3, last30Days: 1 }]);
    expect(invokeMock).toHaveBeenCalledWith('get_mod_downloads');
  });

  it('turns offline, malformed answers and rejections into null', async () => {
    const stats = createTauriDownloadStats();
    invokeMock.mockResolvedValueOnce(null);
    expect(await stats.totals()).toBeNull();
    invokeMock.mockResolvedValueOnce([{ id: 'a', total: -1, last30Days: 0 }]);
    expect(await stats.totals()).toBeNull();
    invokeMock.mockRejectedValueOnce(new Error('down'));
    expect(await stats.totals()).toBeNull();
  });

  it('reads and validates the 30-day series of one mod', async () => {
    invokeMock.mockResolvedValueOnce(series);
    expect(await createTauriDownloadStats().daily('a')).toEqual(series);
    expect(invokeMock).toHaveBeenCalledWith('get_mod_download_daily', { id: 'a' });
  });

  it('turns an offline, short or rejected series into null', async () => {
    const stats = createTauriDownloadStats();
    invokeMock.mockResolvedValueOnce(null);
    expect(await stats.daily('a')).toBeNull();
    invokeMock.mockResolvedValueOnce(series.slice(1));
    expect(await stats.daily('a')).toBeNull();
    invokeMock.mockRejectedValueOnce(new Error('down'));
    expect(await stats.daily('a')).toBeNull();
  });
});
