import { describe, expect, it } from 'vitest';
import type { LauncherState, SessionPhase } from '../controller';
import { selectProfileLocked } from '../selectProfileLocked';

const state = (session: SessionPhase): LauncherState => ({ session }) as LauncherState;

describe('selectProfileLocked', () => {
  it.each([
    { kind: 'starting' },
    { kind: 'running', since: null },
    { kind: 'stopping', since: null },
    { kind: 'cleaning-up' },
  ] as const)('is locked while %o', (session) => {
    expect(selectProfileLocked(state(session))).toBe(true);
  });

  it.each([
    { kind: 'idle' },
    { kind: 'ended', cleaned: 1 },
    { kind: 'never-started', cleaned: 0 },
    { kind: 'unavailable' },
  ] as const)('is not locked when %o', (session) => {
    expect(selectProfileLocked(state(session))).toBe(false);
  });
});
