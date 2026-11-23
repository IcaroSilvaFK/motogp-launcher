import type { Notice } from '../application';
import { BannerIcon } from './BannerIcon';
import { useI18n } from './i18n';
import type { Messages } from './i18n';
import { useLauncher } from './LauncherContext';
import { StaleSessionNotice } from './StaleSessionNotice';

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
  }
}

export function NoticeBar() {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const { error, notice } = state;

  return (
    <div className="notices">
      <StaleSessionNotice />
      {error !== null && (
        <div role="alert" className="notice notice-error">
          <BannerIcon variant="error" />
          <p>
            {t.errors[error.code]}
            {error.message !== '' && <span className="notice-detail"> {error.message}</span>}
          </p>
          <button type="button" className="btn btn-ghost" onClick={() => controller.dismissError()}>
            {t.dismiss}
          </button>
        </div>
      )}
      {error === null && notice !== null && (
        <div role="status" className="notice notice-success">
          <BannerIcon variant="success" />
          <p>{noticeText(t, notice)}</p>
          <button type="button" className="btn btn-ghost" onClick={() => controller.dismissNotice()}>
            {t.dismiss}
          </button>
        </div>
      )}
    </div>
  );
}
