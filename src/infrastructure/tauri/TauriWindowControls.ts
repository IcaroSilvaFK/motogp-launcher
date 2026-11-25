import { getCurrentWindow } from '@tauri-apps/api/window';
import type { WindowControls } from '../../application';
import { subscribe } from '../subscription';

/** Window controls over the current Tauri window (the custom title bar, issue #69). */
export function createTauriWindowControls(): WindowControls {
  return {
    minimize: () => getCurrentWindow().minimize(),
    toggleMaximize: () => getCurrentWindow().toggleMaximize(),
    close: () => getCurrentWindow().close(),
    isMaximized: () => getCurrentWindow().isMaximized(),
    onMaximizedChange: (listener) => {
      const win = getCurrentWindow();
      // A resize is the only signal Tauri gives for maximize/restore, so the state is read again.
      const register = (deliver: (maximized: boolean) => void) =>
        win
          .onResized(() => {
            win.isMaximized().then(deliver, (error: unknown) => {
              console.warn('Could not read the window state', error);
            });
          })
          .catch((error: unknown) => {
            console.warn('Could not listen to window resizes', error);
            return () => undefined;
          });
      return subscribe(register, listener);
    },
  };
}
