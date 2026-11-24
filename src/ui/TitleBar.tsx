import { useEffect, useState } from 'react';
import type { WindowControls } from '../application';
import { BrandMark, WindowGlyph } from './Icon';
import { useI18n } from './i18n';

/** Runs a window call and only logs a failure (sync throw or rejection): the launcher never crashes. */
function attempt(call: () => Promise<unknown>) {
  try {
    void call().catch((error: unknown) => console.warn('Window control failed', error));
  } catch (error) {
    console.warn('Window control failed', error);
  }
}

/**
 * Custom title bar (issue #69), drawn because the native decorations are off. The brand and title
 * are a Tauri drag region (drag and the native double-click-to-maximize); the caption buttons are
 * not. Without a WindowControls port (plain browser, tests) only the title is shown.
 */
export function TitleBar({ controls }: { controls?: WindowControls }) {
  const { t } = useI18n();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!controls) return;
    let active = true;
    const update = (value: boolean) => {
      if (active) setMaximized(value);
    };
    let unsubscribe = () => {};
    try {
      void controls.isMaximized().then(update, (error: unknown) => {
        console.warn('Could not read the window state', error);
      });
      unsubscribe = controls.onMaximizedChange(update);
    } catch (error) {
      console.warn('Could not follow the window state', error);
    }
    return () => {
      active = false;
      unsubscribe();
    };
  }, [controls]);

  const maximizeLabel = maximized ? t.titleBar.restore : t.titleBar.maximize;

  return (
    <div className="titlebar">
      <div className="titlebar-drag" data-tauri-drag-region="">
        <BrandMark className="titlebar-mark" data-tauri-drag-region="" />
        <span className="titlebar-title" data-tauri-drag-region="">
          {t.appName}
        </span>
      </div>
      {controls && (
        <div className="titlebar-controls" role="group" aria-label={t.titleBar.controls}>
          <button
            type="button"
            className="titlebar-button"
            aria-label={t.titleBar.minimize}
            title={t.titleBar.minimize}
            onClick={() => attempt(() => controls.minimize())}
          >
            <WindowGlyph name="minimize" />
          </button>
          <button
            type="button"
            className="titlebar-button"
            aria-label={maximizeLabel}
            title={maximizeLabel}
            onClick={() => attempt(() => controls.toggleMaximize())}
          >
            <WindowGlyph name={maximized ? 'restore' : 'maximize'} />
          </button>
          <button
            type="button"
            className="titlebar-button titlebar-close"
            aria-label={t.titleBar.close}
            title={t.titleBar.close}
            onClick={() => attempt(() => controls.close())}
          >
            <WindowGlyph name="close" />
          </button>
        </div>
      )}
    </div>
  );
}
