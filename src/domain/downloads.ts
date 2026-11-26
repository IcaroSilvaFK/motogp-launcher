// Download counts (#120). The backend reports successful catalog installs to the community server
// and reads the counts back; the UI only sees these validated shapes. `null` always means "no
// data" (offline, a missing command, a malformed answer): the UI then shows nothing.

/** Lifetime and 30-day downloads of one catalog mod, as `get_mod_downloads` answers. */
export interface DownloadTotal {
  id: string;
  total: number;
  last30Days: number;
}

/** One UTC day of downloads; `date` is `YYYY-MM-DD`. */
export interface DailyDownloads {
  date: string;
  count: number;
}

/** Whether the launcher may report successful installs. Unset means on (decided in #120). */
export type UsagePreference = 'on' | 'off';

/** The chart window: `get_mod_download_daily` answers exactly this many days, oldest first. */
export const DOWNLOAD_DAYS = 30;

const DAY = /^\d{4}-\d{2}-\d{2}$/;

const isCount = (value: unknown): value is number => Number.isSafeInteger(value) && (value as number) >= 0;

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;

/** A real calendar day in `YYYY-MM-DD` form (rejects `2026-02-31`). */
function isDay(value: unknown): value is string {
  if (typeof value !== 'string' || !DAY.test(value)) return false;
  const parsed = new Date(`${value}T00:00:00Z`);
  return !Number.isNaN(parsed.getTime()) && parsed.toISOString().slice(0, 10) === value;
}

/** Validates an untrusted IPC answer; anything malformed is `null`, never a partial list. */
export function parseDownloadTotals(value: unknown): DownloadTotal[] | null {
  if (!Array.isArray(value)) return null;
  const totals: DownloadTotal[] = [];
  for (const item of value) {
    if (!isRecord(item)) return null;
    const { id, total, last30Days } = item;
    if (typeof id !== 'string' || id === '' || !isCount(total) || !isCount(last30Days)) return null;
    totals.push({ id, total, last30Days });
  }
  return totals;
}

/** Validates the 30-day series: exactly 30 real days, strictly ascending, non-negative counts. */
export function parseDownloadDaily(value: unknown): DailyDownloads[] | null {
  if (!Array.isArray(value) || value.length !== DOWNLOAD_DAYS) return null;
  const series: DailyDownloads[] = [];
  let previous = '';
  for (const item of value) {
    if (!isRecord(item)) return null;
    const { date, count } = item;
    // ISO days compare correctly as strings; any real day is greater than ''.
    if (!isDay(date) || !isCount(count) || date <= previous) return null;
    series.push({ date, count });
    previous = date;
  }
  return series;
}

/** The chart's text summary: downloads in the window and the busiest day (the latest on a tie). */
export function summarizeDaily(series: readonly DailyDownloads[]): { total: number; peak: DailyDownloads | null } {
  let total = 0;
  let peak: DailyDownloads | null = null;
  for (const day of series) {
    total += day.count;
    if (day.count > 0 && (peak === null || day.count >= peak.count)) peak = day;
  }
  return { total, peak };
}

/** Whether a successful install may be reported. */
export function usageSharingFor(preference: UsagePreference): boolean {
  return preference === 'on';
}
