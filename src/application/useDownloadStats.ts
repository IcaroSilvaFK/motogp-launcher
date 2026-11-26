import { useEffect, useState } from 'react';
import type { DailyDownloads, DownloadTotal } from '../domain';
import type { DownloadStats } from './ports/DownloadStats';

/** Calls the port and folds every failure (rejection or a synchronous throw) into `null`. */
function settle<T>(read: () => Promise<T | null>, done: (value: T | null) => void): void {
  let request: Promise<T | null>;
  try {
    request = read();
  } catch {
    request = Promise.resolve(null);
  }
  request.then(done, () => done(null));
}

/**
 * The download totals indexed by mod id (#120), requested once `enabled` turns true. `null` while
 * loading, offline or without a port: the UI then shows no count at all.
 */
export function useDownloadTotals(
  stats: DownloadStats | undefined,
  enabled: boolean,
): ReadonlyMap<string, DownloadTotal> | null {
  const [totals, setTotals] = useState<ReadonlyMap<string, DownloadTotal> | null>(null);

  useEffect(() => {
    if (!stats || !enabled) return;
    let active = true;
    settle(
      () => stats.totals(),
      (list) => {
        if (active && list) setTotals(new Map(list.map((total) => [total.id, total])));
      },
    );
    return () => {
      active = false;
    };
  }, [stats, enabled]);

  return totals;
}

/**
 * The 30-day series of one mod (#120). `null` while loading, offline or without a port; switching
 * mods drops the previous series at once and ignores its late answer.
 */
export function useDownloadDaily(stats: DownloadStats | undefined, id: string): DailyDownloads[] | null {
  // Tagged with the mod it belongs to, so a series is never shown for another mod.
  const [series, setSeries] = useState<{ id: string; days: DailyDownloads[] } | null>(null);

  useEffect(() => {
    if (!stats) return;
    let active = true;
    settle(
      () => stats.daily(id),
      (days) => {
        if (active) setSeries(days ? { id, days } : null);
      },
    );
    return () => {
      active = false;
    };
  }, [stats, id]);

  return series !== null && series.id === id ? series.days : null;
}
