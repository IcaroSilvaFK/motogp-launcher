import { useId } from 'react';
import { useDownloadDaily } from '../application';
import { summarizeDaily } from '../domain';
import { formatCount } from './DownloadCounts';
import { useDownloadStats } from './DownloadStatsContext';
import { useI18n } from './i18n';

// Plot geometry in viewBox units. The SVG stretches horizontally (preserveAspectRatio="none") and
// its CSS height is `--chart-height`, which equals PLOT_HEIGHT, so vertical units stay 1:1 px.
const SLOT = 10;
const BAR = 7;
const PLOT_HEIGHT = 64;
const BASELINE = 2;

/** A UTC day ("2026-11-20") as a short local date ("Nov 20", "20 de nov."). */
function formatDay(day: string, locale: string): string {
  return new Intl.DateTimeFormat(locale, { month: 'short', day: 'numeric', timeZone: 'UTC' }).format(
    new Date(`${day}T00:00:00Z`),
  );
}

/**
 * The last 30 days of downloads of one mod (#120): one neutral bar per UTC day, oldest first.
 * Zero days keep a faint baseline so the window reads as 30 days. Offline (or without a port) the
 * whole section is absent: no reserved space and no error.
 */
export function DownloadChart({ id }: { id: string }) {
  const { t, locale } = useI18n();
  const { stats } = useDownloadStats();
  const series = useDownloadDaily(stats, id);
  const titleId = useId();
  const summaryId = useId();
  const first = series?.[0];
  const last = series?.[series.length - 1];
  if (!series || !first || !last) return null;

  const { total, peak } = summarizeDaily(series);
  const max = Math.max(1, ...series.map((day) => day.count));
  const summary = t.downloads.chartSummary(
    total,
    formatCount(total, locale),
    peak === null ? null : { count: formatCount(peak.count, locale), date: formatDay(peak.date, locale) },
  );

  return (
    <section aria-labelledby={titleId} className="mod-section download-chart">
      <h3 id={titleId} className="group-title">
        {t.downloads.chartTitle}
      </h3>
      <figure className="download-chart-figure">
        <figcaption id={summaryId} className="hint">
          {summary}
        </figcaption>
        <svg
          role="img"
          aria-labelledby={summaryId}
          className="download-chart-plot"
          viewBox={`0 0 ${series.length * SLOT} ${PLOT_HEIGHT}`}
          preserveAspectRatio="none"
        >
          {series.map((day, index) => {
            const x = index * SLOT;
            const height = day.count === 0 ? 0 : Math.max(BASELINE, (day.count / max) * PLOT_HEIGHT);
            return (
              <g
                key={day.date}
                className="download-chart-day"
                data-day={day.date}
                data-empty={day.count === 0 ? '' : undefined}
              >
                <title>{t.downloads.bar(formatDay(day.date, locale), day.count, formatCount(day.count, locale))}</title>
                {/* The whole column is the hover target, so thin bars stay easy to point at. */}
                <rect className="download-chart-hit" x={x} y={0} width={SLOT} height={PLOT_HEIGHT} />
                <rect
                  className="download-chart-baseline"
                  x={x + (SLOT - BAR) / 2}
                  y={PLOT_HEIGHT - BASELINE}
                  width={BAR}
                  height={BASELINE}
                />
                {height > 0 && (
                  <rect
                    className="download-chart-bar"
                    x={x + (SLOT - BAR) / 2}
                    y={PLOT_HEIGHT - height}
                    width={BAR}
                    height={height}
                  />
                )}
              </g>
            );
          })}
        </svg>
        <div className="download-chart-axis" aria-hidden="true">
          <span>{formatDay(first.date, locale)}</span>
          <span>{formatDay(last.date, locale)}</span>
        </div>
      </figure>
    </section>
  );
}
