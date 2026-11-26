import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { DailyDownloads, DownloadTotal } from '../../domain';
import type { DownloadStats } from '../ports/DownloadStats';
import { useDownloadDaily, useDownloadTotals } from '../useDownloadStats';

const TOTALS: DownloadTotal[] = [{ id: 'a', total: 10, last30Days: 4 }];
const series = (count: number): DailyDownloads[] =>
  Array.from({ length: 30 }, (_, index) => ({
    date: new Date(Date.UTC(2026, 9, 28 + index)).toISOString().slice(0, 10),
    count,
  }));

function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>((done) => (resolve = done));
  return { promise, resolve };
}

function port(patch: Partial<DownloadStats> = {}): DownloadStats {
  return {
    report: vi.fn(() => Promise.resolve()),
    totals: vi.fn(() => Promise.resolve(TOTALS)),
    daily: vi.fn(() => Promise.resolve(series(1))),
    ...patch,
  };
}

describe('useDownloadTotals', () => {
  it('requests nothing without a port or before it is enabled', () => {
    const stats = port();
    const { result: none } = renderHook(() => useDownloadTotals(undefined, true));
    const { result: idle } = renderHook(() => useDownloadTotals(stats, false));
    expect(none.current).toBeNull();
    expect(idle.current).toBeNull();
    expect(stats.totals).not.toHaveBeenCalled();
  });

  it('indexes the totals by mod id once enabled', async () => {
    const stats = port();
    const { result } = renderHook(() => useDownloadTotals(stats, true));
    await waitFor(() => expect(result.current?.get('a')).toEqual(TOTALS[0]));
    expect(stats.totals).toHaveBeenCalledTimes(1);
  });

  it('stays null when offline, on a rejection or on a throwing port', async () => {
    const offline = port({ totals: vi.fn(() => Promise.resolve(null)) });
    const rejected = port({ totals: vi.fn(() => Promise.reject(new Error('down'))) });
    const thrown = port({
      totals: vi.fn(() => {
        throw new Error('not wired');
      }),
    });
    for (const stats of [offline, rejected, thrown]) {
      const { result } = renderHook(() => useDownloadTotals(stats, true));
      await act(async () => undefined);
      expect(result.current).toBeNull();
    }
  });

  it('ignores an answer that arrives after unmount', async () => {
    const answer = deferred<DownloadTotal[] | null>();
    const stats = port({ totals: vi.fn(() => answer.promise) });
    const { result, unmount } = renderHook(() => useDownloadTotals(stats, true));
    unmount();
    await act(async () => answer.resolve(TOTALS));
    expect(result.current).toBeNull();
  });
});

describe('useDownloadDaily', () => {
  it('loads the series of the mod', async () => {
    const stats = port();
    const { result } = renderHook(() => useDownloadDaily(stats, 'a'));
    await waitFor(() => expect(result.current).toHaveLength(30));
    expect(stats.daily).toHaveBeenCalledWith('a');
  });

  it('requests nothing without a port', () => {
    const { result } = renderHook(() => useDownloadDaily(undefined, 'a'));
    expect(result.current).toBeNull();
  });

  it('is null when offline, on a rejection or on a throwing port', async () => {
    const offline = port({ daily: vi.fn(() => Promise.resolve(null)) });
    const rejected = port({ daily: vi.fn(() => Promise.reject(new Error('down'))) });
    const thrown = port({
      daily: vi.fn(() => {
        throw new Error('not wired');
      }),
    });
    for (const stats of [offline, rejected, thrown]) {
      const { result } = renderHook(() => useDownloadDaily(stats, 'a'));
      await act(async () => undefined);
      expect(result.current).toBeNull();
    }
  });

  it('drops the previous mod at once and ignores its late answer', async () => {
    const first = deferred<DailyDownloads[] | null>();
    const second = deferred<DailyDownloads[] | null>();
    const stats = port({ daily: vi.fn((id: string) => (id === 'a' ? first.promise : second.promise)) });
    const { result, rerender } = renderHook(({ id }) => useDownloadDaily(stats, id), { initialProps: { id: 'a' } });
    rerender({ id: 'b' });
    await act(async () => second.resolve(series(2)));
    await act(async () => first.resolve(series(9)));
    expect(result.current?.[0]?.count).toBe(2);
  });

  it('shows nothing for the new mod until its own answer arrives', async () => {
    const stats = port({
      daily: vi.fn((id: string) => (id === 'a' ? Promise.resolve(series(1)) : new Promise<null>(() => undefined))),
    });
    const { result, rerender } = renderHook(({ id }) => useDownloadDaily(stats, id), { initialProps: { id: 'a' } });
    await waitFor(() => expect(result.current).not.toBeNull());
    rerender({ id: 'b' });
    expect(result.current).toBeNull();
  });
});
