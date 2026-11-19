import { useSyncExternalStore } from 'react';
import type { LauncherController, LauncherState } from './controller';

export function useLauncherState(controller: LauncherController): LauncherState {
  return useSyncExternalStore(controller.subscribe, controller.getState, controller.getState);
}
