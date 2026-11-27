import { describe, expect, it } from 'vitest';
import type { LauncherState } from '../controller';
import { selectRestoreBlock } from '../selectRestoreBlock';

const game = { name: 'MotoGP 25', root: 'D:/g', mods_dir: 'D:/g/~mods', app_id: 1 };

function state(overrides: Partial<LauncherState>): LauncherState {
  return { session: { kind: 'idle' }, recovery: null, game, staleSession: null, ...overrides } as LauncherState;
}

describe('selectRestoreBlock', () => {
  it('is not blocked when idle with a configured game', () => {
    expect(selectRestoreBlock(state({}))).toBeNull();
  });

  it.each([
    { kind: 'starting' },
    { kind: 'running', since: null },
    { kind: 'stopping', since: null },
    { kind: 'cleaning-up' },
    { kind: 'unavailable' },
  ] as const)('is blocked by the session while it is %s', (session) => {
    expect(selectRestoreBlock(state({ session: session as LauncherState['session'] }))).toBe('session');
  });

  it.each(['game-running', 'not-run'] as const)('is blocked by the session when recovery is %s', (s) => {
    expect(selectRestoreBlock(state({ recovery: { state: s, error: null, cleaned_count: 0 } }))).toBe('session');
  });

  it.each(['recovered', 'failed'] as const)('is not blocked when recovery is %s (restore is the retry)', (s) => {
    expect(selectRestoreBlock(state({ recovery: { state: s, error: null, cleaned_count: 0 } }))).toBeNull();
  });

  it('is blocked without a configured game', () => {
    expect(selectRestoreBlock(state({ game: null }))).toBe('no-game');
  });

  it('is blocked by a stale session from another install', () => {
    expect(selectRestoreBlock(state({ staleSession: { modsDir: 'E:/old', tracked: 2 } }))).toBe('stale');
  });
});
