import type {
  Config,
  DeploySession,
  GameInfo,
  Locale,
  ModEntry,
  Profile,
  RecoveryStatus,
  SessionErrorEvent,
  SessionEvent,
} from '../domain';

export type Unsubscribe = () => void;

/** One method per backend command. Rejects with a CommandError. */
export interface LauncherGateway {
  getConfig(): Promise<Config>;
  detectGames(): Promise<GameInfo[]>;
  setGameDir(path: string): Promise<GameInfo>;
  getGame(): Promise<GameInfo | null>;
  listMods(): Promise<ModEntry[]>;
  importMods(paths: string[]): Promise<ModEntry[]>;
  removeMod(id: string): Promise<void>;
  listProfiles(): Promise<Profile[]>;
  createProfile(name: string): Promise<Profile>;
  renameProfile(id: string, name: string): Promise<Profile>;
  deleteProfile(id: string): Promise<void>;
  setProfileMods(id: string, modIds: string[]): Promise<Profile>;
  setActiveProfile(id: string): Promise<void>;
  play(profileId: string): Promise<void>;
  recoveryStatus(): Promise<RecoveryStatus>;
  deploySession(): Promise<DeploySession>;
  forgetStaleSession(): Promise<void>;
  /** Cleans the stale folder; `confirmedDir` must equal the manifest's folder. Resolves with the files removed. */
  cleanStaleSession(confirmedDir: string): Promise<number>;
}

/** `null` means the user cancelled the dialog. */
export interface FilePicker {
  pickModFiles(): Promise<string[] | null>;
  pickGameDirectory(): Promise<string | null>;
}

/** Both methods reject when the subscription cannot be established (no event will ever be delivered). */
export interface SessionEvents {
  onSession(callback: (event: SessionEvent) => void): Promise<Unsubscribe>;
  onSessionError(callback: (event: SessionErrorEvent) => void): Promise<Unsubscribe>;
}

export interface DropEvents {
  onDrop(callback: (paths: string[]) => void): Promise<Unsubscribe>;
}

/** Synchronous on purpose: the locale must be known before the first render. */
export interface LocaleStore {
  load(): Locale | null;
  save(locale: Locale): void;
}

/** Whether the sidebar is collapsed. Synchronous so the first render already has the right width. */
export interface NavPreferenceStore {
  load(): boolean | null;
  save(collapsed: boolean): void;
}
