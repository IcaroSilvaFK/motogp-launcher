import type { LauncherState } from './controller';
import { selectProfileLocked } from './selectProfileLocked';

/** False while a game session locks the library: catalog installs add mods, so they wait for the session to end. */
export function selectCanInstall(state: LauncherState): boolean {
  return !selectProfileLocked(state);
}
