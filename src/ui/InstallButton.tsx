import { useId } from 'react';
import { selectCanInstall } from '../application';
import type { CatalogInstallState } from '../domain';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

export interface InstallButtonProps {
  state: CatalogInstallState;
  /** True while a game session locks the library: Install and Retry are unavailable (with a reason). */
  disabled?: boolean;
  onInstall(): void;
  onRetry(): void;
  onCancel(): void;
}

const IDLE: CatalogInstallState = { status: 'idle' };

/**
 * Compact install control for one catalog entry (#74): Install, then a linear progress with
 * Cancel while downloading, an indeterminate bar while verifying/installing, a confirmed
 * "Installed" badge, or the translated failure with Retry. Presentational only.
 */
export function InstallButton({ state, disabled = false, onInstall, onRetry, onCancel }: InstallButtonProps) {
  const { t } = useI18n();
  const hintId = useId();
  const lockedReason = t.errors.library_locked_by_session;
  const lockProps = disabled ? { 'aria-disabled': true as const, 'aria-describedby': hintId, title: lockedReason } : {};
  const hint = disabled ? (
    <span id={hintId} className="visually-hidden">
      {lockedReason}
    </span>
  ) : null;

  switch (state.status) {
    case 'idle':
      return (
        <div className="install-control" data-status="idle">
          <button
            type="button"
            className="btn btn-accent-outline btn-sm"
            {...lockProps}
            onClick={() => {
              if (!disabled) onInstall();
            }}
          >
            <Icon name="import" />
            {t.catalogInstall.install}
          </button>
          {hint}
        </div>
      );
    case 'downloading': {
      const percent = Math.min(Math.max(Math.round(state.percent), 0), 100);
      const label = t.catalogInstall.downloading(percent);
      return (
        <div className="install-control" data-status="downloading" aria-busy="true">
          <div className="install-progress-line">
            <span className="install-progress-label num">{label}</span>
            <button type="button" className="btn btn-ghost btn-sm" onClick={onCancel}>
              {t.catalogInstall.cancel}
            </button>
          </div>
          <div
            className="install-progress"
            role="progressbar"
            aria-label={t.catalogInstall.progressLabel}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={percent}
            aria-valuetext={label}
          >
            <span className="install-progress-fill" style={{ width: `${percent}%` }} />
          </div>
        </div>
      );
    }
    case 'verifying':
    case 'importing': {
      const label = state.status === 'verifying' ? t.catalogInstall.verifying : t.catalogInstall.installing;
      return (
        <div className="install-control" data-status={state.status} aria-busy="true">
          <div className="install-progress-line">
            <span className="install-progress-label">{label}</span>
          </div>
          <div
            className="install-progress"
            role="progressbar"
            aria-label={t.catalogInstall.progressLabel}
            aria-valuetext={label}
            data-indeterminate=""
          >
            <span className="install-progress-fill" />
          </div>
        </div>
      );
    }
    case 'installed':
      return (
        <div className="install-control" data-status="installed">
          <span className="badge badge-on install-done" role="status">
            <Icon name="check" />
            {t.catalogInstall.installed}
          </span>
        </div>
      );
    case 'failed': {
      const message = (t.errors as Partial<Record<string, string>>)[state.code] ?? t.catalogInstall.failed;
      return (
        <div className="install-control" data-status="failed">
          <p className="install-error" role="alert">
            {message}
          </p>
          <button
            type="button"
            className="btn btn-sm"
            {...lockProps}
            onClick={() => {
              if (!disabled) onRetry();
            }}
          >
            <Icon name="refresh" />
            {t.catalogInstall.retry}
          </button>
          {hint}
        </div>
      );
    }
  }
}

/** InstallButton wired to the launcher: the entry's install state, the session lock and the controller. */
export function CatalogInstallButton({ entryId }: { entryId: string }) {
  const { controller, state } = useLauncher();
  const start = () => void controller.installCatalogEntry(entryId);
  return (
    <InstallButton
      state={state.catalogInstalls[entryId] ?? IDLE}
      disabled={!selectCanInstall(state)}
      onInstall={start}
      onRetry={start}
      onCancel={() => void controller.cancelCatalogInstall(entryId)}
    />
  );
}
