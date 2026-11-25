import type { UpdateState } from '../application';
import { BannerIcon } from './BannerIcon';
import { useI18n } from './i18n';
import { usePresence } from './usePresence';

export interface UpdateBannerProps {
  state: UpdateState;
  installBlocked: boolean;
  onUpdate: () => void;
}

type VisibleUpdateState = Exclude<UpdateState, { kind: 'idle' } | { kind: 'none' }>;

function visible(state: UpdateState): VisibleUpdateState | null {
  return state.kind === 'idle' || state.kind === 'none' ? null : state;
}

export function UpdateBanner({ state: current, installBlocked, onUpdate }: UpdateBannerProps) {
  const { t } = useI18n();
  // The banner keeps its last visible state while it plays its exit animation.
  const presence = usePresence(visible(current));
  const state = presence.item;
  if (state === null) return null;

  const info = state.info;
  const busy = state.kind === 'downloading' || state.kind === 'installed';
  const canUpdate = !busy && !installBlocked;
  const exiting = presence.state === 'exiting';

  return (
    <section
      className={
        state.kind === 'error'
          ? 'update-banner update-banner-error banner banner-error'
          : 'update-banner banner banner-info'
      }
      aria-labelledby={info ? 'update-title' : undefined}
      data-state={presence.state}
      inert={exiting}
      aria-hidden={exiting || undefined}
    >
      <BannerIcon variant={state.kind === 'error' ? 'error' : 'info'} />
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
        <button type="button" className="btn btn-primary" disabled={!canUpdate} onClick={onUpdate}>
          {t.update.button}
        </button>
      )}
    </section>
  );
}
