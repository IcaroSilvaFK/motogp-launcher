import type { LauncherState } from './controller';

/** True while a game session is starting or running: the active profile's mods must not change. */
export function selectProfileLocked(state: LauncherState): boolean {
  return state.session.kind === 'starting' || state.session.kind === 'running';
}
