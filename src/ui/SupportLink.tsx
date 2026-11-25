import { useCallback, useEffect, useId, useRef, useState } from 'react';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

/**
 * Opens the support page through the controller and remembers whether the last try failed. The
 * address is a backend constant (#94): the UI never holds or passes a URL. A new try clears the
 * failure; an answer that arrives after unmount is ignored.
 */
function useSupportPage() {
  const { controller } = useLauncher();
  const [failed, setFailed] = useState(false);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const open = useCallback(() => {
    setFailed(false);
    controller.openSupportPage().catch(() => {
      if (mounted.current) setFailed(true);
    });
  }, [controller]);

  return { open, failed };
}

/**
 * "Support the project" (#94), self-contained so it can sit next to future footer slots.
 *
 * - `sidebar`: a quiet footer entry styled like the navigation (icon + label); collapsed, only the
 *   icon shows, with the label as accessible name and tooltip.
 * - `settings`: a Settings row (label + short text on the left, an outlined button on the right).
 *
 * A failure to open the browser is reported inline (`role="alert"`), never in a dialog.
 */
export function SupportLink({
  placement,
  collapsed = false,
}: {
  placement: 'sidebar' | 'settings';
  collapsed?: boolean;
}) {
  const { t } = useI18n();
  const { open, failed } = useSupportPage();
  const descriptionId = useId();
  const errorId = useId();
  const describedBy = failed ? `${descriptionId} ${errorId}` : descriptionId;

  const description = (
    <span id={descriptionId} hidden>
      {t.support.opensInBrowser}
    </span>
  );

  if (placement === 'sidebar') {
    return (
      <div className="support-link" data-failed={String(failed)}>
        <button
          type="button"
          className="support-entry"
          title={collapsed ? (failed ? `${t.support.label}: ${t.support.openFailed}` : t.support.label) : undefined}
          aria-describedby={describedBy}
          onClick={open}
        >
          <Icon name="heart" className="nav-icon" />
          <span className={collapsed ? 'visually-hidden' : 'nav-label'}>{t.support.label}</span>
          {!collapsed && <Icon name="external" className="support-entry-external" />}
        </button>
        {description}
        {failed && (
          <p id={errorId} className={collapsed ? 'visually-hidden' : 'support-inline-error'} role="alert">
            {t.support.openFailed}
          </p>
        )}
      </div>
    );
  }

  return (
    <div className="setting-row support-row">
      <div className="setting-text">
        <p className="setting-label">{t.support.label}</p>
        <p className="hint">{t.support.aboutText}</p>
        {failed && (
          <p id={errorId} className="support-inline-error" role="alert">
            {t.support.openFailed}
          </p>
        )}
      </div>
      <div className="setting-control">
        <button type="button" className="btn btn-sm" aria-describedby={describedBy} onClick={open}>
          <Icon name="heart" />
          {t.support.button}
        </button>
        {description}
      </div>
    </div>
  );
}
