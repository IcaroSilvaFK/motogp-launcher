import { createContext, useContext } from 'react';
import type { Diagnostics, UpdateController } from '../application';

/** Launcher-wide facts shared by the status bar and Settings > About. */
export interface AppInfoContextValue {
  /** The running launcher version; null while unknown or when it cannot be read. */
  version: string | null;
  /** Absent in tests and dev builds without the updater. */
  updater?: UpdateController;
  /** Log folder and copy-diagnostics actions (#148); absent in tests and dev builds without them. */
  diagnostics?: Diagnostics;
  /**
   * True once the user ran a manual check in this app session. Lives above the screens so it
   * survives leaving Settings, and so a silent startup check (which also ends in `none`) never
   * reads as "up to date".
   */
  userChecked: boolean;
  markUserChecked: () => void;
}

export const AppInfoContext = createContext<AppInfoContextValue>({
  version: null,
  userChecked: false,
  markUserChecked: () => {},
});

export function useAppInfo(): AppInfoContextValue {
  return useContext(AppInfoContext);
}
