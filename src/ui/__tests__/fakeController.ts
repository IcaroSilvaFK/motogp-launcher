import { vi } from 'vitest';
import type { LauncherController, LauncherState } from '../../application';

export const initialState: LauncherState = {
  status: 'ready',
  config: { game_dir: null, active_profile: null },
  game: null,
  mods: [],
  profiles: [{ id: 'vanilla', name: 'Vanilla', mod_ids: [] }],
  detected: null,
  busy: false,
  session: { kind: 'idle' },
  recovery: null,
  staleSession: null,
  error: null,
  notice: null,
};

export interface FakeController extends LauncherController {
  setState(patch: Partial<LauncherState>): void;
  counters: { start: number; dispose: number; subscribeDrop: number; release: number };
  release: ReturnType<typeof vi.fn>;
}

/** A fake controller: mutable state, listeners notified on setState, spies for every action. */
export function createFakeController(patch: Partial<LauncherState> = {}): FakeController {
  let state: LauncherState = { ...initialState, ...patch };
  const listeners = new Set<() => void>();
  const counters = { start: 0, dispose: 0, subscribeDrop: 0, release: 0 };
  const release = vi.fn(() => {
    counters.release += 1;
  });
  const action = () => vi.fn(() => Promise.resolve());

  return {
    getState: () => state,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    setState(next) {
      state = { ...state, ...next };
      listeners.forEach((listener) => listener());
    },
    counters,
    release,
    start: vi.fn(() => {
      counters.start += 1;
      return Promise.resolve();
    }),
    dispose: vi.fn(() => {
      counters.dispose += 1;
    }),
    detectGames: action(),
    chooseGameDirectory: action(),
    useGame: action(),
    importFromDialog: action(),
    importPaths: action(),
    subscribeDrop: vi.fn(() => {
      counters.subscribeDrop += 1;
      return Promise.resolve(release);
    }),
    removeMod: action(),
    createProfile: action(),
    renameProfile: action(),
    deleteProfile: action(),
    setProfileMods: action(),
    setActiveProfile: action(),
    toggleModInActiveProfile: action(),
    forgetStaleSession: action(),
    cleanStaleSession: action(),
    play: action(),
    dismissError: vi.fn(),
    dismissNotice: vi.fn(),
  };
}
