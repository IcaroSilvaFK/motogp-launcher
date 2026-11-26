import type {
  Catalog,
  CatalogInstallProgress,
  CatalogInstallResult,
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
  /** Opens the project's support page in the default browser; the backend owns the URL. */
  openSupportPage(): Promise<void>;
  // Online catalog (#72): fetched by the backend, cached on disk, available offline.
  /** The catalog while the cache is fresh, otherwise a fetch (an older copy is flagged `stale` when offline). */
  getCatalog(): Promise<Catalog>;
  /** Forces a fetch, still bounded by the backend's backoff after a failure. */
  refreshCatalog(): Promise<Catalog>;
  // Catalog pages (#73).
  /** Opens the entry's web page in the default browser. Only the id is sent: the backend owns the URL and its policy. */
  openCatalogPage(id: string): Promise<void>;
  /** Downloads, verifies and imports a catalog entry, optionally enabling it in the active profile (#129). */
  installCatalogEntry(id: string, enableAfterInstall: boolean): Promise<CatalogInstallResult>;
  /** Cancels the in-flight install of the entry; a no-op when nothing is in flight. */
  cancelCatalogInstall(id: string): Promise<void>;
  /** Progress of catalog installs. Synchronous: a failed registration only means no progress is shown. */
  onCatalogInstallProgress(handler: (progress: CatalogInstallProgress) => void): Unsubscribe;
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

/** How the Discover results are laid out. */
export type DiscoverView = 'grid' | 'list';

/** The Discover grid/list choice. Synchronous so the first render already uses the stored layout. */
export interface DiscoverViewStore {
  load(): DiscoverView | null;
  save(view: DiscoverView): void;
}

/** The sponsored-content choice. `null` means the user never chose; `load` may throw (blocked storage). */
export interface AdPreferenceStore {
  load(): 'on' | 'off' | null;
  save(value: 'on' | 'off'): void;
}

/** "Enable mods after installing from Discover" (#129). `null` means never chosen (default on); `load` may throw. */
export interface AutoEnablePreferenceStore {
  load(): boolean | null;
  save(value: boolean): void;
}

/** The anonymous usage choice (#120). `null` means never chosen (sharing is on); `load` may throw. */
export interface UsagePreferenceStore {
  load(): 'on' | 'off' | null;
  save(value: 'on' | 'off'): void;
}
