import type { LauncherState } from './controller';

/** True when no game session is active and no cleanup is pending, so an update may be installed.
 *  An unavailable session subscription means the launcher cannot know, so it is treated as not clean. */
export function selectSessionClean(state: LauncherState): boolean {
  const { kind } = state.session;
  if (
    kind === 'starting' ||
    kind === 'running' ||
    kind === 'stopping' ||
    kind === 'cleaning-up' ||
    kind === 'unavailable'
  )
    return false;
  const recovery = state.recovery?.state;
  return recovery !== 'game-running' && recovery !== 'failed';
}
