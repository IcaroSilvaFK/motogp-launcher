import { useState } from 'react';
import { BannerIcon } from './BannerIcon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';
import { usePresence } from './usePresence';
import type { PresenceState } from './usePresence';

/** Offers to forget or clean the files a previous game folder still holds. */
export function StaleSessionNotice() {
  const { state } = useLauncher();
  // The panel unmounts with the session (after its exit animation), so an open confirmation
  // never outlives it.
  const presence = usePresence(state.staleSession);
  return presence.item === null ? null : <StaleSessionPanel staleSession={presence.item} motion={presence.state} />;
}

function StaleSessionPanel({
  staleSession,
  motion,
}: {
  staleSession: { modsDir: string; tracked: number };
  motion: PresenceState;
}) {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const { busy } = state;
  const [confirming, setConfirming] = useState(false);
  const text = t.staleSession;
  const exiting = motion === 'exiting';

  return (
    <section
      role="region"
      aria-label={text.regionLabel}
      className="notice notice-warning stale-session"
      data-state={motion}
      inert={exiting}
      aria-hidden={exiting || undefined}
    >
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
