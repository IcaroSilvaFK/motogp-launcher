import type { UpdateState } from '../application';
import { useI18n } from './i18n';

export interface UpdateBannerProps {
  state: UpdateState;
  installBlocked: boolean;
  onUpdate: () => void;
}

export function UpdateBanner({ state, installBlocked, onUpdate }: UpdateBannerProps) {
  const { t } = useI18n();
  if (state.kind === 'idle' || state.kind === 'none') return null;

  const info = state.info;
  const busy = state.kind === 'downloading' || state.kind === 'installed';
  const canUpdate = !busy && !installBlocked;

  return (
    <section
      className={state.kind === 'error' ? 'update-banner update-banner-error' : 'update-banner'}
      aria-labelledby={info ? 'update-title' : undefined}
    >
      {info && (
        <>
          <h2 id="update-title">{t.update.available(info.version)}</h2>
          <p className="hint">{t.update.currentVersion(info.currentVersion)}</p>
        </>
      )}
      {info?.notes && state.kind === 'available' && <p className="update-notes">{info.notes}</p>}
      {state.kind === 'downloading' && (
        <>
          <p>{t.update.downloading}</p>
          {state.total !== null ? (
            <progress aria-label={t.update.progressLabel} max={state.total} value={state.downloaded} />
          ) : (
            <progress aria-label={t.update.progressLabel} />
          )}
        </>
      )}
      {state.kind === 'installed' && <p role="status">{t.update.restarting}</p>}
      {state.kind === 'error' && <p role="alert">{t.update.errors[state.error.kind]}</p>}
      {installBlocked && !busy && <p className="hint">{t.update.blockedBySession}</p>}
      {info !== null && (
        <button type="button" disabled={!canUpdate} onClick={onUpdate}>
          {t.update.button}
        </button>
      )}
    </section>
  );
}
