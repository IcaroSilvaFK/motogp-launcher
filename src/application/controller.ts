import type {
  CommandError,
  Config,
  FileConflict,
  GameInfo,
  ModEntry,
  Profile,
  RecoveryStatus,
  StorageUsage,
} from '../domain';
import type { DropEvents, FilePicker, LauncherGateway, SessionEvents, Unsubscribe } from './ports';

export type SessionPhase =
  | { kind: 'idle' }
  | { kind: 'starting' }
  | { kind: 'running' }
  | { kind: 'ended'; cleaned: number }
  | { kind: 'never-started'; cleaned: number }
  | { kind: 'unavailable' };

export type Notice =
  | { kind: 'imported'; count: number }
  | { kind: 'mod-removed' }
  | { kind: 'game-configured'; name: string }
  | { kind: 'profile-saved' }
  | { kind: 'recovered'; cleaned: number }
  | { kind: 'game-running' }
  | { kind: 'stale-forgotten' }
  | { kind: 'stale-cleaned'; cleaned: number };

export interface LauncherState {
  status: 'loading' | 'ready';
  config: Config;
  game: GameInfo | null;
  mods: ModEntry[];
  profiles: Profile[];
  /** Conflicts of the active profile load order; empty without an active profile or mods. */
  conflicts: FileConflict[];
  /** `null` until detection has run. */
  detected: GameInfo[] | null;
  busy: boolean;
  session: SessionPhase;
  recovery: RecoveryStatus | null;
  /** Set when the deploy manifest points to another game install and needs the user's decision. */
  staleSession: { modsDir: string; tracked: number } | null;
  error: CommandError | null;
  notice: Notice | null;
}

export interface LauncherController {
  getState(): LauncherState;
  /** useSyncExternalStore-compatible: returns the unsubscribe function. */
  subscribe(listener: () => void): Unsubscribe;
  /** Subscribes to session events first, then recovery_status, then loads data. */
  start(): Promise<void>;
  /** Releases every listener exactly once. */
  dispose(): void;
  detectGames(): Promise<void>;
  /** Opens the folder dialog; cancelling does nothing. */
  chooseGameDirectory(): Promise<void>;
  useGame(path: string): Promise<void>;
  /** Opens the import dialog; cancelling does not import. */
  importFromDialog(): Promise<void>;
  importPaths(paths: string[]): Promise<void>;
  subscribeDrop(): Promise<Unsubscribe>;
  removeMod(id: string): Promise<void>;
  createProfile(name: string): Promise<void>;
  renameProfile(id: string, name: string): Promise<void>;
  deleteProfile(id: string): Promise<void>;
  setProfileMods(id: string, modIds: string[]): Promise<void>;
  setActiveProfile(id: string): Promise<void>;
  /**
   * Adds the mod to (or removes it from) the active profile, silently (no notice). No-op without an
   * active profile, on Vanilla, or while a session is starting or running.
   */
  toggleModInActiveProfile(modId: string): Promise<void>;
  /** Conflicts of an unsaved draft order; leaves state untouched and resolves `[]` on failure. */
  previewConflicts(modIds: string[]): Promise<FileConflict[]>;
  /** Drops the stale manifest without touching files; no-op without a stale session. */
  forgetStaleSession(): Promise<void>;
  /** Cleans the stale folder, confirming the folder shown in the notice; no-op without a stale session. */
  cleanStaleSession(): Promise<void>;
  play(): Promise<void>;
  /**
   * Measures the launcher data folder. A plain pass-through: no state change, no global error;
   * rejects with a CommandError so the caller shows the failure inline.
   */
  getStorageUsage(): Promise<StorageUsage>;
  /** Opens the launcher data folder; rejects with a CommandError, no global error. */
  openDataFolder(): Promise<void>;
  dismissError(): void;
  dismissNotice(): void;
}

export interface LauncherDeps {
  gateway: LauncherGateway;
  picker: FilePicker;
  session: SessionEvents;
  drop: DropEvents;
}

/** Implemented by the application layer (Task C): can the user start a session now? */
export type SelectCanPlay = (state: LauncherState) => boolean;
