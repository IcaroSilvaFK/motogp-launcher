import type { LauncherState } from './controller';

/** True while a game session is live (starting, running, stopping or cleaning up): the active profile's mods must not change. */
export function selectProfileLocked(state: LauncherState): boolean {
  const { kind } = state.session;
  return kind === 'starting' || kind === 'running' || kind === 'stopping' || kind === 'cleaning-up';
}
