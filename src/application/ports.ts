import type {
  Catalog,
  Config,
  FileConflict,
  CurrentSession,
  DeploySession,
  GameInfo,
  Locale,
  ModEntry,
  Profile,
  RecoveryStatus,
  SessionErrorEvent,
  SessionEvent,
  StorageUsage,
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
  /** Stateless: conflicts of a candidate load order (first loads first, last wins). */
  listConflicts(modIds: string[]): Promise<FileConflict[]>;
  play(profileId: string): Promise<void>;
  recoveryStatus(): Promise<RecoveryStatus>;
  /** The live session phase; lets a reloaded UI resume an ongoing session. */
  currentSession(): Promise<CurrentSession>;
  /** Terminates the game; the normal cleanup follows through the session events. */
  stopSession(): Promise<void>;
  deploySession(): Promise<DeploySession>;
  forgetStaleSession(): Promise<void>;
  /** Cleans the stale folder; `confirmedDir` must equal the manifest's folder. Resolves with the files removed. */
  cleanStaleSession(confirmedDir: string): Promise<number>;
  /** Measures the launcher data folder (read-only; never waits for a running mutation). */
  getStorageUsage(): Promise<StorageUsage>;
  /** The Steam artwork of the configured game as a `data:` URL; `null` when there is none. */
  getGameArtwork(): Promise<string | null>;
  /** Opens the launcher data folder in the system file manager; the backend decides the path. */
  openDataFolder(): Promise<void>;
  // Online catalog (#72): fetched by the backend, cached on disk, available offline.
  /** The catalog while the cache is fresh, otherwise a fetch (an older copy is flagged `stale` when offline). */
  getCatalog(): Promise<Catalog>;
  /** Forces a fetch, still bounded by the backend's backoff after a failure. */
  refreshCatalog(): Promise<Catalog>;
  // Catalog pages (#73).
  /** Opens the entry's web page in the default browser. Only the id is sent: the backend owns the URL and its policy. */
  openCatalogPage(id: string): Promise<void>;
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
