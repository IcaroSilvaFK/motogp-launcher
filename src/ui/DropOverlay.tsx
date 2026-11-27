import { useEffect } from 'react';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';
import { usePresence } from './usePresence';

/**
 * Full-window drop target shown while files are dragged over the Library (#152). It never takes
 * pointer events (the OS delivers the drop to the webview) and Esc dismisses it for this drag.
 */
export function DropOverlay() {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const shown = usePresence(state.dragging ? true : null);

  useEffect(() => {
    if (!state.dragging) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') controller.cancelDrag();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [controller, state.dragging]);

  if (shown.item === null) return null;
  const exiting = shown.state === 'exiting';
  return (
    <div
      className="drop-overlay"
      role="status"
      aria-labelledby="drop-overlay-title"
      data-state={shown.state}
      aria-hidden={exiting || undefined}
    >
      <div className="drop-overlay-target">
        <Icon name="import" className="drop-overlay-icon" />
        <p id="drop-overlay-title" className="drop-overlay-title">
          {t.library.dropOverlayTitle}
        </p>
        <p className="drop-overlay-detail">{t.library.dropOverlayDetail}</p>
      </div>
    </div>
  );
}
