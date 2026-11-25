import type { LauncherState } from './controller';

/** True while the game session is starting or running, so it can be stopped. */
export function selectCanStop(state: LauncherState): boolean {
  return state.session.kind === 'starting' || state.session.kind === 'running';
}
