import { createContext, useCallback, useContext, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { useDownloadTotals } from '../application';
import type { DownloadStats, UsagePreferenceHandle } from '../application';
import type { DownloadTotal, UsagePreference } from '../domain';

/** Download counts and the usage choice (#120), wired in `main.tsx`. */
export interface DownloadStatsValue {
  /** Absent in tests and builds without the community: no count and no chart render. */
  stats?: DownloadStats;
  /** Totals by mod id; `null` until requested and loaded, and whenever the server is out of reach. */
  totals: ReadonlyMap<string, DownloadTotal> | null;
  /** Asks for the totals; the first caller triggers the single request of the session. */
  requestTotals(): void;
  /** The usage switch; absent without a preference handle (no Settings row). */
  usage?: { preference: UsagePreference; setPreference(value: UsagePreference): void };
}

const DownloadStatsContext = createContext<DownloadStatsValue>({ totals: null, requestTotals: () => undefined });

export function DownloadStatsProvider({
  stats,
  usagePreference,
  children,
}: {
  stats?: DownloadStats;
  usagePreference?: UsagePreferenceHandle;
  children: ReactNode;
}) {
  // Requested lazily: nothing reaches the server until a screen shows a count.
  const [requested, setRequested] = useState(false);
  const totals = useDownloadTotals(stats, requested);
  const requestTotals = useCallback(() => setRequested(true), []);
  const [preference, setPreferenceState] = useState<UsagePreference>(() => usagePreference?.get() ?? 'off');
  // The save stays out of the state updater, which StrictMode may run twice.
  const setPreference = useCallback(
    (value: UsagePreference) => {
      setPreferenceState(value);
      usagePreference?.set(value);
    },
    [usagePreference],
  );
  const value = useMemo<DownloadStatsValue>(
    () => ({
      stats,
      totals,
      requestTotals,
      usage: usagePreference ? { preference, setPreference } : undefined,
    }),
    [stats, totals, requestTotals, usagePreference, preference, setPreference],
  );
  return <DownloadStatsContext.Provider value={value}>{children}</DownloadStatsContext.Provider>;
}

export function useDownloadStats(): DownloadStatsValue {
  return useContext(DownloadStatsContext);
}
