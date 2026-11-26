import { usageSharingFor } from '../../domain';
import type { UsagePreference } from '../../domain';
import type { DownloadStats } from '../ports/DownloadStats';

export interface ReportCatalogDownloadDeps {
  stats: DownloadStats;
  preference: { get(): UsagePreference };
}

/**
 * Counts a successful catalog install (#120) when the user shares anonymous usage. Fire-and-forget:
 * returns at once, never throws, and a failed report is silent.
 */
export function reportCatalogDownload({ stats, preference }: ReportCatalogDownloadDeps, id: string): void {
  try {
    if (!usageSharingFor(preference.get())) return;
    stats.report(id).catch(() => undefined);
  } catch {
    // A broken preference or port must never surface in the install flow.
  }
}
