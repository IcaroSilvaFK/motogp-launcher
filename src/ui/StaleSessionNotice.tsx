import { useState } from 'react';
import { BannerIcon } from './BannerIcon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

/** Offers to forget or clean the files a previous game folder still holds. */
export function StaleSessionNotice() {
  const { state } = useLauncher();
  // The panel unmounts with the session, so an open confirmation never outlives it.
  return state.staleSession === null ? null : <StaleSessionPanel staleSession={state.staleSession} />;
}

function StaleSessionPanel({ staleSession }: { staleSession: { modsDir: string; tracked: number } }) {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const { busy } = state;
  const [confirming, setConfirming] = useState(false);
  const text = t.staleSession;

  return (
    <section role="region" aria-label={text.regionLabel} className="notice notice-warning stale-session">
      <BannerIcon variant="warning" />
      <div>
        {confirming ? (
          <>
            <p>{text.confirmPrompt}</p>
            <p>
              {text.folderLabel} <code>{staleSession.modsDir}</code>
            </p>
          </>
        ) : (
          <>
            <p>
              <strong>{text.title}</strong>
            </p>
            <p>{text.description(staleSession.tracked)}</p>
            <p>
              {text.folderLabel} <code>{staleSession.modsDir}</code>
            </p>
          </>
        )}
      </div>
      <div className="notice-actions">
        {confirming ? (
          <>
            <button
              className="btn btn-ghost"
              type="button"
              disabled={busy}
              onClick={() => {
                setConfirming(false);
                void controller.cleanStaleSession();
              }}
            >
              {text.confirm}
            </button>
            <button type="button" className="btn btn-ghost" disabled={busy} onClick={() => setConfirming(false)}>
              {text.cancel}
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              className="btn btn-ghost"
              disabled={busy}
              onClick={() => void controller.forgetStaleSession()}
            >
              {text.forget}
            </button>
            <button type="button" className="btn btn-ghost" disabled={busy} onClick={() => setConfirming(true)}>
              {text.clean}
            </button>
          </>
        )}
      </div>
    </section>
  );
}
