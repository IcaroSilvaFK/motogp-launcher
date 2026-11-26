import { useId } from 'react';
import { useDownloadStats } from './DownloadStatsContext';
import { useI18n } from './i18n';

/**
 * Settings > Preferences: the anonymous usage switch (#120). On unless the user turns it off; the
 * hint states exactly what a report carries. Absent when no preference handle is wired.
 */
export function UsageSharingRow() {
  const { t } = useI18n();
  const { usage } = useDownloadStats();
  const labelId = useId();
  const hintId = useId();
  if (!usage) return null;
  const on = usage.preference === 'on';
  return (
    <div className="setting-row">
      <div className="setting-text">
        <span id={labelId} className="setting-label">
          {t.settings.shareUsage}
        </span>
        <p id={hintId} className="hint">
          {t.settings.shareUsageHint}
        </p>
      </div>
      <div className="setting-control">
        <span className="toggle">
          <button
            type="button"
            role="switch"
            className="switch"
            aria-checked={on}
            aria-labelledby={labelId}
            aria-describedby={hintId}
            onClick={() => usage.setPreference(on ? 'off' : 'on')}
          >
            <span className="switch-thumb" aria-hidden="true" />
          </button>
          <span className={on ? 'toggle-label toggle-label-on' : 'toggle-label'} aria-hidden="true">
            {on ? t.settings.shareUsageOn : t.settings.shareUsageOff}
          </span>
        </span>
      </div>
    </div>
  );
}
