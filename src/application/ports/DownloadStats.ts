import type { DailyDownloads, DownloadTotal } from '../../domain';

/**
 * Download counts (#120). The backend talks to the community server and owns the random install
 * id; the UI only sends the catalog mod id. None of the methods reject in practice, but callers
 * still treat a rejection as offline.
 */
export interface DownloadStats {
  /** Counts one successful install of the catalog mod. Fire-and-forget: a failure is silent. */
  report(id: string): Promise<void>;
  /** The counts of every mod with downloads, or `null` when the server is out of reach. */
  totals(): Promise<DownloadTotal[] | null>;
  /** The last 30 UTC days of one mod, oldest first, or `null`. */
  daily(id: string): Promise<DailyDownloads[] | null>;
}
