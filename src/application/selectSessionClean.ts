import type { LauncherState } from './controller';

/** True when no game session is active and no cleanup is pending, so an update may be installed.
 *  An unavailable session subscription means the launcher cannot know, so it is treated as not clean. */
export function selectSessionClean(state: LauncherState): boolean {
  if (state.session.kind === 'starting' || state.session.kind === 'running' || state.session.kind === 'unavailable')
    return false;
  const recovery = state.recovery?.state;
  return recovery !== 'game-running' && recovery !== 'failed';
}
