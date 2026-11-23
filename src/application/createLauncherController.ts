import type { CommandError } from '../domain';
import { VANILLA_ID, isSupportedModFile, isVanilla, toCommandError, toggleMod } from '../domain';
import type { LauncherController, LauncherDeps, LauncherState, Notice } from './controller';
import { selectCanPlay } from './selectCanPlay';
import { selectProfileLocked } from './selectProfileLocked';
import type { Unsubscribe } from './ports';

type Reload = 'config' | 'game' | 'mods' | 'profiles';

const initialState: LauncherState = {
  status: 'loading',
  config: { game_dir: null, active_profile: null },
  game: null,
  mods: [],
  profiles: [],
  conflicts: [],
  detected: null,
  busy: false,
  session: { kind: 'idle' },
  recovery: null,
  staleSession: null,
  error: null,
  notice: null,
};

/** Calls the gateway without touching the state; any rejection becomes a CommandError for the caller. */
async function passThrough<T>(call: () => Promise<T>): Promise<T> {
  try {
    return await call();
  } catch (caught) {
    throw toCommandError(caught);
  }
}

/** Framework-free observable store holding the launcher state and its use cases. */
export function createLauncherController(deps: LauncherDeps): LauncherController {
  const { gateway, picker, session, drop } = deps;
  let state: LauncherState = initialState;
  const listeners = new Set<() => void>();
  let startPromise: Promise<void> | null = null;
  let disposed = false;
  /** Bumped on every (re)start so a superseded `runStart` releases what it subscribed. */
  let generation = 0;
  const releases: Unsubscribe[] = [];

  function set(patch: Partial<LauncherState>): void {
    state = { ...state, ...patch };
    for (const listener of [...listeners]) listener();
  }

  /** Keeps a subscription; if the controller was disposed meanwhile, releases it at once. */
  function track(release: Unsubscribe, run: number): void {
    if (disposed || run !== generation) release();
    else releases.push(release);
  }

  async function load(what: Reload[]): Promise<void> {
    const wanted = new Set(what);
    const patch: Partial<LauncherState> = {};
    const tasks: Array<Promise<void>> = [];
    if (wanted.has('config')) {
      tasks.push(gateway.getConfig().then((config) => void (patch.config = config)));
    }
    if (wanted.has('game')) {
      tasks.push(
        gateway.getGame().then(async (game) => {
          patch.game = game;
          // The deploy session only matters against a configured game; skip the backend otherwise.
          if (game === null) {
            patch.staleSession = null;
            return;
          }
          const deploy = await gateway.deploySession();
          patch.staleSession =
            deploy.state === 'stale' && deploy.mods_dir !== null
              ? { modsDir: deploy.mods_dir, tracked: deploy.tracked }
              : null;
        }),
      );
    }
    if (wanted.has('mods')) {
      tasks.push(gateway.listMods().then((mods) => void (patch.mods = mods)));
    }
    if (wanted.has('profiles')) {
      tasks.push(gateway.listProfiles().then((profiles) => void (patch.profiles = profiles)));
    }
    const results = await Promise.allSettled(tasks);
    set(patch);
    const failed = results.find((r): r is PromiseRejectedResult => r.status === 'rejected');
    if (failed) throw failed.reason;
  }

  /** Recomputes the conflicts of the active profile load order; on failure clears them and rethrows. */
  async function refreshConflicts(): Promise<void> {
    const active = state.profiles.find((profile) => profile.id === state.config.active_profile);
    const modIds = active?.mod_ids ?? [];
    if (modIds.length === 0) {
      set({ conflicts: [] });
      return;
    }
    try {
      set({ conflicts: await gateway.listConflicts(modIds) });
    } catch (caught) {
      set({ conflicts: [] });
      throw caught;
    }
  }

  /** Runs a mutation: busy flag, error normalization, and a reload that always happens. */
  async function mutate(
    reload: Reload[],
    action: () => Promise<Notice | null | undefined>,
    withConflicts = false,
  ): Promise<void> {
    set({ busy: true, error: null, notice: null });
    let notice: Notice | null = null;
    let error: CommandError | null = null;
    try {
      notice = (await action()) ?? null;
    } catch (caught) {
      error = toCommandError(caught);
    }
    try {
      if (reload.length > 0) await load(reload);
    } catch (caught) {
      error ??= toCommandError(caught);
    }
    if (withConflicts) {
      try {
        await refreshConflicts();
      } catch (caught) {
        error ??= toCommandError(caught);
      }
    }
    set({ busy: false, error, notice: error ? null : notice });
  }

  /** Subscribes to the session events; on failure releases what was obtained and returns the error. */
  async function subscribeSession(run: number): Promise<CommandError | null> {
    const obtained: Unsubscribe[] = [];
    try {
      obtained.push(
        await session.onSession((event) => {
          if (event.state === 'started') set({ session: { kind: 'running' } });
          else set({ session: { kind: event.state, cleaned: event.cleaned } });
        }),
      );
      obtained.push(
        await session.onSessionError((event) => {
          set({ session: { kind: 'idle' }, error: { code: 'session_error', message: event.message } });
        }),
      );
    } catch (caught) {
      for (const release of obtained) release();
      return { code: 'session_subscription_failed', message: toCommandError(caught).message };
    }
    for (const release of obtained) track(release, run);
    return null;
  }

  async function runStart(run: number): Promise<void> {
    const subscriptionError = await subscribeSession(run);
    if (disposed || run !== generation) return;

    let error: CommandError | null = null;
    let notice: Notice | null = null;
    let recovery = null;
    try {
      recovery = await gateway.recoveryStatus();
      if (recovery.state === 'failed') {
        error = { code: 'recovery_failed', message: recovery.error ?? '' };
      } else if (recovery.state === 'recovered' && recovery.cleaned_count > 0) {
        notice = { kind: 'recovered', cleaned: recovery.cleaned_count };
      } else if (recovery.state === 'game-running') {
        notice = { kind: 'game-running' };
      }
      await load(['config', 'game', 'mods', 'profiles']);
    } catch (caught) {
      error ??= toCommandError(caught);
    }
    try {
      await refreshConflicts();
    } catch (caught) {
      error ??= toCommandError(caught);
    }
    // The stale-session notice explains a failed recovery caused by a manifest from another install.
    if (error?.code === 'recovery_failed' && state.staleSession !== null) error = null;
    set({
      status: 'ready',
      recovery,
      // The subscription failure is the most fundamental error, so it wins over recovery/load errors.
      error: subscriptionError ?? error ?? state.error,
      notice: notice ?? state.notice,
      ...(subscriptionError ? { session: { kind: 'unavailable' as const } } : {}),
    });
  }

  async function importPaths(paths: string[]): Promise<void> {
    const supported = paths.filter(isSupportedModFile);
    if (supported.length === 0) return;
    await mutate(
      ['mods'],
      async () => {
        const imported = await gateway.importMods(supported);
        return { kind: 'imported', count: imported.length };
      },
      true,
    );
  }

  async function configureGame(path: string): Promise<void> {
    await mutate(['config', 'game'], async () => {
      const game = await gateway.setGameDir(path);
      return { kind: 'game-configured', name: game.name };
    });
  }

  return {
    getState: () => state,
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    start() {
      // A start after dispose (React StrictMode remount) begins a fresh run.
      if (startPromise === null || disposed) {
        disposed = false;
        generation += 1;
        startPromise = runStart(generation);
      }
      return startPromise;
    },
    dispose() {
      disposed = true;
      for (const release of releases.splice(0)) release();
    },
    async detectGames() {
      await mutate([], async () => {
        set({ detected: await gateway.detectGames() });
        return null;
      });
    },
    async chooseGameDirectory() {
      const path = await picker.pickGameDirectory();
      if (path === null) return;
      await configureGame(path);
    },
    useGame: configureGame,
    async importFromDialog() {
      const paths = await picker.pickModFiles();
      if (paths === null) return;
      await importPaths(paths);
    },
    importPaths,
    async subscribeDrop() {
      const release = await drop.onDrop((paths) => {
        void importPaths(paths);
      });
      return release;
    },
    removeMod: (id) =>
      mutate(
        ['mods', 'profiles'],
        async () => {
          await gateway.removeMod(id);
          return { kind: 'mod-removed' };
        },
        true,
      ),
    createProfile: (name) =>
      mutate(['profiles'], async () => {
        await gateway.createProfile(name);
        return null;
      }),
    renameProfile: (id, name) =>
      mutate(['profiles'], async () => {
        await gateway.renameProfile(id, name);
        return null;
      }),
    deleteProfile: (id) =>
      mutate(
        ['profiles', 'config'],
        async () => {
          await gateway.deleteProfile(id);
          return null;
        },
        true,
      ),
    setProfileMods: (id, modIds) =>
      mutate(
        ['profiles'],
        async () => {
          await gateway.setProfileMods(id, modIds);
          return { kind: 'profile-saved' };
        },
        true,
      ),
    toggleModInActiveProfile: async (modId) => {
      const active = state.profiles.find((p) => p.id === state.config.active_profile);
      if (!active || isVanilla(active) || selectProfileLocked(state)) return;
      await mutate(
        ['profiles'],
        async () => {
          await gateway.setProfileMods(active.id, toggleMod(active.mod_ids, modId));
          return null;
        },
        true,
      );
    },
    setActiveProfile: (id) =>
      mutate(
        ['config'],
        async () => {
          await gateway.setActiveProfile(id);
          return null;
        },
        true,
      ),
    async previewConflicts(modIds) {
      if (modIds.length === 0) return [];
      try {
        return await gateway.listConflicts(modIds);
      } catch (caught) {
        set({ error: toCommandError(caught) });
        return [];
      }
    },
    async forgetStaleSession() {
      if (state.staleSession === null) return;
      await mutate(['game'], async () => {
        await gateway.forgetStaleSession();
        return { kind: 'stale-forgotten' };
      });
    },
    async cleanStaleSession() {
      const stale = state.staleSession;
      if (stale === null) return;
      await mutate(['game'], async () => {
        const cleaned = await gateway.cleanStaleSession(stale.modsDir);
        return { kind: 'stale-cleaned', cleaned };
      });
    },
    async play() {
      if (!selectCanPlay(state)) return;
      set({ error: null, notice: null, session: { kind: 'starting' } });
      try {
        await gateway.play(state.config.active_profile ?? VANILLA_ID);
      } catch (caught) {
        set({ session: { kind: 'idle' }, error: toCommandError(caught) });
      }
    },
    getStorageUsage: () => passThrough(() => gateway.getStorageUsage()),
    openDataFolder: () => passThrough(() => gateway.openDataFolder()),
    dismissError() {
      set({ error: null });
    },
    dismissNotice() {
      set({ notice: null });
    },
  };
}
