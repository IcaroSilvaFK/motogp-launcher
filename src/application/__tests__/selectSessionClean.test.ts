import { describe, expect, it } from 'vitest';
import type { LauncherState } from '../controller';
import { selectSessionClean } from '../selectSessionClean';

function state(overrides: Partial<LauncherState>): LauncherState {
  return { session: { kind: 'idle' }, recovery: null, ...overrides } as LauncherState;
}

describe('selectSessionClean', () => {
  it('is clean when idle with no recovery pending', () => {
    expect(selectSessionClean(state({}))).toBe(true);
  });

  it.each([
    { kind: 'ended', cleaned: 1 },
    { kind: 'never-started', cleaned: 0 },
  ] as const)('is clean after %o', (session) => {
    expect(selectSessionClean(state({ session }))).toBe(true);
  });

  it.each([
    { kind: 'starting' },
    { kind: 'running', since: null },
    { kind: 'stopping', since: null },
    { kind: 'cleaning-up' },
    { kind: 'unavailable' },
  ] as const)('is not clean while %o', (session) => {
    expect(selectSessionClean(state({ session }))).toBe(false);
  });

  it.each(['game-running', 'failed'] as const)('is not clean when recovery is %s', (s) => {
    expect(selectSessionClean(state({ recovery: { state: s, error: null, cleaned_count: 0 } }))).toBe(false);
  });

  it.each(['not-run', 'recovered'] as const)('is clean when recovery is %s', (s) => {
    expect(selectSessionClean(state({ recovery: { state: s, error: null, cleaned_count: 0 } }))).toBe(true);
  });
});
