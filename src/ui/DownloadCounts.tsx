import { useEffect } from 'react';
import type { DownloadTotal } from '../domain';
import { useDownloadStats } from './DownloadStatsContext';
import { useI18n } from './i18n';

/** Formats a count with the grouping of the UI locale ("1,234" / "1.234"). */
export function formatCount(value: number, locale: string): string {
  return new Intl.NumberFormat(locale).format(value);
}

/**
 * The totals of one mod (#120). `null` (render nothing) while loading, offline or without a port.
 * A mod missing from a loaded list has no downloads yet: the server only lists counted mods.
 */
function useModTotal(id: string): DownloadTotal | null {
  const { stats, totals, requestTotals } = useDownloadStats();
  useEffect(() => {
    if (stats) requestTotals();
  }, [stats, requestTotals]);
  if (totals === null) return null;
  return totals.get(id) ?? { id, total: 0, last30Days: 0 };
}

/** The count on a Discover card's meta line, after a separator dot. Nothing when unknown. */
export function DownloadCount({ id }: { id: string }) {
  const { t, locale } = useI18n();
  const total = useModTotal(id);
  if (total === null) return null;
  return (
    <>
      <span className="dot" aria-hidden="true" />
      <span className="num download-count">{t.downloads.count(total.total, formatCount(total.total, locale))}</span>
    </>
  );
}

/** The "Downloads" cell of the mod page stats strip. Nothing when unknown, so no space is reserved. */
export function DownloadStat({ id }: { id: string }) {
  const { t, locale } = useI18n();
  const total = useModTotal(id);
  if (total === null) return null;
  return (
    <div className="mod-stat">
      <dt>{t.downloads.label}</dt>
      <dd>
        <span className="num">{formatCount(total.total, locale)}</span>
        <span className="mod-stat-secondary">{t.downloads.last30Days(formatCount(total.last30Days, locale))}</span>
      </dd>
    </div>
  );
}
