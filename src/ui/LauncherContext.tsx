import { createContext, useContext } from 'react';
import type { LauncherController, LauncherState } from '../application';

export interface LauncherContextValue {
  controller: LauncherController;
  state: LauncherState;
}

export const LauncherContext = createContext<LauncherContextValue | null>(null);

export function useLauncher(): LauncherContextValue {
  const value = useContext(LauncherContext);
  if (value === null) throw new Error('useLauncher must be used inside <App>');
  return value;
}
