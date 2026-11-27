import { useEffect } from 'react';
import type { Notice } from '../application';
import { BannerIcon } from './BannerIcon';
import { useI18n } from './i18n';
import type { Messages } from './i18n';
import { useLauncher } from './LauncherContext';
import { StaleSessionNotice } from './StaleSessionNotice';
import { usePresence } from './usePresence';

function noticeText(t: Messages, notice: Notice): string {
  switch (notice.kind) {
    case 'imported':
      return t.notices.imported(notice.count);
    case 'mod-removed':
      return t.notices.modRemoved;
    case 'game-configured':
      return t.notices.gameConfigured(notice.name);
    case 'profile-saved':
      return t.notices.profileSaved;
    case 'recovered':
      return t.notices.recovered(notice.cleaned);
    case 'game-running':
      return t.notices.gameRunning;
    case 'stale-forgotten':
      return t.notices.staleForgotten;
    case 'stale-cleaned':
      return t.notices.staleCleaned(notice.cleaned);
    case 'mod-installed':
      if (notice.conflictWith !== undefined) return t.notices.modInstalledConflict(notice.name, notice.conflictWith);
      return notice.enabled ? t.notices.modInstalledEnabled(notice.name) : t.notices.modInstalled(notice.name);
  }
}

/** Translated text for an error code; an unexpected code (newer backend) falls back to the generic text. */
export function errorText(t: Messages, code: string): string {
  const known: Partial<Record<string, string>> = t.errors;
  return known[code] ?? t.errors.unknown;
}

export function NoticeBar({ onOpenLibrary }: { onOpenLibrary?: () => void } = {}) {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  // Each message stays mounted for its exit animation; an exiting one is inert (no focus, no clicks).
  const shownError = usePresence(state.error);
  const shownNotice = usePresence(state.error === null ? state.notice : null);
  const error = shownError.item;
  const notice = shownNotice.item;

  // The technical detail is for developers: it goes to the console once per shown error, never to the UI.
  useEffect(() => {
    if (error !== null) console.warn(`Launcher error ${error.code}: ${error.message}`);
  }, [error]);

  return (
    <div className="notices">
      <StaleSessionNotice />
      {error !== null && (
        <div
          role="alert"
          className="notice notice-error"
          data-state={shownError.state}
          inert={shownError.state === 'exiting'}
          aria-hidden={shownError.state === 'exiting' || undefined}
        >
          <BannerIcon variant="error" />
          <p>{errorText(t, error.code)}</p>
          <button type="button" className="btn btn-ghost" onClick={() => controller.dismissError()}>
            {t.dismiss}
          </button>
        </div>
      )}
      {notice !== null && (
        <div
          role="status"
          className="notice notice-success"
          data-state={shownNotice.state}
          inert={shownNotice.state === 'exiting'}
          aria-hidden={shownNotice.state === 'exiting' || undefined}
        >
          <BannerIcon variant="success" />
          <p>{noticeText(t, notice)}</p>
          {notice.kind === 'mod-installed' && notice.enabled && (
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() => {
                void controller.toggleModInActiveProfile(notice.modId);
                controller.dismissNotice();
              }}
            >
              {t.notices.undo}
            </button>
          )}
          {notice.kind === 'mod-installed' && onOpenLibrary && (
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() => {
                onOpenLibrary();
                controller.dismissNotice();
              }}
            >
              {t.notices.openLibrary}
            </button>
          )}
          <button type="button" className="btn btn-ghost" onClick={() => controller.dismissNotice()}>
            {t.dismiss}
          </button>
        </div>
      )}
    </div>
  );
}
