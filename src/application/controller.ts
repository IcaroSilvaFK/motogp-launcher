import type { CommandError, Config, GameInfo, ModEntry, Profile, RecoveryStatus } from '../domain';
import type { DropEvents, FilePicker, LauncherGateway, SessionEvents, Unsubscribe } from './ports';

export type SessionPhase =
  | { kind: 'idle' }
  | { kind: 'starting' }
  | { kind: 'running' }
  | { kind: 'ended'; cleaned: number }
  | { kind: 'never-started'; cleaned: number };

export type Notice =
  | { kind: 'imported'; count: number }
  | { kind: 'mod-removed' }
  | { kind: 'game-configured'; name: string }
  | { kind: 'profile-saved' }
  | { kind: 'recovered'; cleaned: number }
  | { kind: 'game-running' };

export interface LauncherState {
  status: 'loading' | 'ready';
  config: Config;
  game: GameInfo | null;
  mods: ModEntry[];
  profiles: Profile[];
  /** `null` until detection has run. */
  detected: GameInfo[] | null;
  busy: boolean;
  session: SessionPhase;
  recovery: RecoveryStatus | null;
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
  play(): Promise<void>;
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
