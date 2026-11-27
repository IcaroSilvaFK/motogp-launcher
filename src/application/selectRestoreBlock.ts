import type { LauncherState } from './controller';

/** Why "Restore original game files" is unavailable, or `null` when it can run. */
export type RestoreBlock = 'session' | 'no-game' | 'stale';

/**
 * Restore removes deployed files, so it waits for any session (or a game still running from before).
 * A failed startup recovery does not block it: restoring is precisely the retry of that cleanup.
 */
export function selectRestoreBlock(state: LauncherState): RestoreBlock | null {
  switch (state.session.kind) {
    case 'starting':
    case 'running':
    case 'stopping':
    case 'cleaning-up':
    case 'unavailable':
      return 'session';
  }
  if (state.recovery?.state === 'game-running' || state.recovery?.state === 'not-run') return 'session';
  if (state.game === null) return 'no-game';
  if (state.staleSession !== null) return 'stale';
  return null;
}
