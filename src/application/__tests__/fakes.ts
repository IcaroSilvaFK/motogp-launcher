import type {
  Catalog,
  CatalogInstallProgress,
  CommandError,
  Config,
  CurrentSession,
  DeploySession,
  FileConflict,
  GameInfo,
  ModEntry,
  Profile,
  RecoveryStatus,
  SessionErrorEvent,
  SessionEvent,
  StorageUsage,
} from '../../domain';
import { VANILLA_ID } from '../../domain';
import type { DropEvents, FilePicker, LauncherGateway, SessionEvents, Unsubscribe } from '../ports';

/** In-memory test doubles for the application ports. Test-only code. */

export interface RecordedCall {
  method: string;
  args: unknown[];
}

export type GatewayMethod = keyof LauncherGateway;

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function modNameFromPath(path: string): string {
  const file = baseName(path);
  const dot = file.lastIndexOf('.');
  return dot > 0 ? file.slice(0, dot) : file;
}

/** A deploy manifest left by a previous session. */
export interface FakeManifest {
  /** The `~mods` folder the manifest points to. */
  mods_dir: string;
  /** Number of files the manifest tracks. */
  tracked: number;
  /** The folder no longer exists on disk. */
  vanished?: boolean;
}

export interface FakeGatewayOptions {
  /** Starts with this manifest: current when `mods_dir` equals the game's, stale otherwise. */
  manifest?: FakeManifest;
  config?: Config;
  game?: GameInfo | null;
  mods?: ModEntry[];
  profiles?: Profile[];
  /** Computes what `listConflicts` returns; defaults to none. */
  conflictsFor?: (modIds: string[]) => FileConflict[];
  detected?: GameInfo[];
  recovery?: RecoveryStatus;
  /** Overrides per path of what `setGameDir` returns (partial GameInfo); e.g. `{ 'C:/g': { app_id: 123 } }`. */
  games?: Record<string, Partial<GameInfo>>;
  /** Or a callback building the GameInfo for any path. Wins over `games`. */
  gameFor?: (path: string) => GameInfo;
  /** Shared log of ordered events (`gateway.<method>`) to assert subscription order across fakes. */
  log?: string[];
  /** What `getStorageUsage` reports; defaults to an empty data folder. */
  storage?: StorageUsage;
  /** What `getGameArtwork` reports while a game is configured (a `data:` URL); defaults to none. */
  artwork?: string | null;
}

const EMPTY_STORAGE: StorageUsage = {
  root: 'C:/launcher-data',
  totalBytes: 0,
  freeBytes: null,
  categories: [
    { category: 'mods', bytes: 0 },
    { category: 'launcher_data', bytes: 0 },
    { category: 'other', bytes: 0 },
  ],
};

export class FakeGateway implements LauncherGateway {
  readonly calls: RecordedCall[] = [];
  config: Config;
  game: GameInfo | null;
  mods: ModEntry[];
  profiles: Profile[];
  detected: GameInfo[];
  recovery: RecoveryStatus;
  /** `~mods` file names "deployed" by play; stays empty when play is refused. */
  deployed: string[] = [];
  /** The persisted deploy manifest; `null` when there is none. */
  manifest: FakeManifest | null;
  /** What `currentSession` reports; tests set it to simulate a session that survived a reload. */
  current: CurrentSession = { phase: 'idle', started_at: null };
  storage: StorageUsage;
  /** The artwork the backend would return for the configured game. */
  artwork: string | null;

  private readonly failures = new Map<string, CommandError>();
  private readonly options: FakeGatewayOptions;
  private readonly log: string[] | undefined;
  private nextProfile = 1;

  constructor(options: FakeGatewayOptions = {}) {
    this.options = options;
    this.log = options.log;
    this.game = options.game ?? null;
    this.config = options.config ?? { game_dir: this.game?.root ?? null, active_profile: VANILLA_ID };
    this.mods = options.mods ? [...options.mods] : [];
    this.profiles = options.profiles ? [...options.profiles] : [{ id: VANILLA_ID, name: 'Vanilla', mod_ids: [] }];
    this.detected = options.detected ?? [];
    this.manifest = options.manifest ? { ...options.manifest } : null;
    this.recovery = options.recovery ?? { state: 'not-run', error: null, cleaned_count: 0 };
    this.storage = options.storage ?? EMPTY_STORAGE;
    this.artwork = options.artwork ?? null;
  }

  /** Makes the next call of `method` reject once with `error`. */
  failNext(method: GatewayMethod, error: CommandError): void {
    this.failures.set(method, error);
  }

  callsOf(method: GatewayMethod): RecordedCall[] {
    return this.calls.filter((call) => call.method === method);
  }

  private async run<T>(method: GatewayMethod, args: unknown[], body: () => T): Promise<T> {
    this.calls.push({ method, args });
    this.log?.push(`gateway.${method}`);
    await Promise.resolve();
    const failure = this.failures.get(method);
    if (failure) {
      this.failures.delete(method);
      throw failure;
    }
    return body();
  }

  getConfig(): Promise<Config> {
    return this.run('getConfig', [], () => ({ ...this.config }));
  }

  detectGames(): Promise<GameInfo[]> {
    return this.run('detectGames', [], () => this.detected.map((game) => ({ ...game })));
  }

  setGameDir(path: string): Promise<GameInfo> {
    return this.run('setGameDir', [path], () => {
      const game = this.buildGame(path);
      this.game = game;
      this.config = { ...this.config, game_dir: path };
      return { ...game };
    });
  }

  getGame(): Promise<GameInfo | null> {
    return this.run('getGame', [], () => (this.game ? { ...this.game } : null));
  }

  listMods(): Promise<ModEntry[]> {
    return this.run('listMods', [], () => this.mods.map((mod) => ({ ...mod })));
  }

  importMods(paths: string[]): Promise<ModEntry[]> {
    return this.run('importMods', [paths], () => {
      const imported: ModEntry[] = [];
      for (const path of paths) {
        const name = modNameFromPath(path);
        let mod = this.mods.find((entry) => entry.id === name);
        if (!mod) {
          mod = { id: name, name, files: [baseName(path)], size_bytes: 1024 };
          this.mods.push(mod);
        }
        imported.push({ ...mod });
      }
      return imported;
    });
  }

  removeMod(id: string): Promise<void> {
    return this.run('removeMod', [id], () => {
      this.mods = this.mods.filter((mod) => mod.id !== id);
      this.profiles = this.profiles.map((profile) => ({
        ...profile,
        mod_ids: profile.mod_ids.filter((modId) => modId !== id),
      }));
    });
  }

  listConflicts(modIds: string[]): Promise<FileConflict[]> {
    return this.run('listConflicts', [[...modIds]], () => this.options.conflictsFor?.(modIds) ?? []);
  }

  listProfiles(): Promise<Profile[]> {
    return this.run('listProfiles', [], () => this.profiles.map((p) => ({ ...p, mod_ids: [...p.mod_ids] })));
  }

  createProfile(name: string): Promise<Profile> {
    return this.run('createProfile', [name], () => {
      const profile: Profile = { id: `profile-${this.nextProfile++}`, name, mod_ids: [] };
      this.profiles.push(profile);
      return { ...profile };
    });
  }

  renameProfile(id: string, name: string): Promise<Profile> {
    return this.run('renameProfile', [id, name], () => {
      const profile = this.requireProfile(id);
      profile.name = name;
      return { ...profile, mod_ids: [...profile.mod_ids] };
    });
  }

  deleteProfile(id: string): Promise<void> {
    return this.run('deleteProfile', [id], () => {
      this.requireProfile(id);
      this.profiles = this.profiles.filter((profile) => profile.id !== id);
    });
  }

  setProfileMods(id: string, modIds: string[]): Promise<Profile> {
    return this.run('setProfileMods', [id, modIds], () => {
      const profile = this.requireProfile(id);
      profile.mod_ids = [...modIds];
      return { ...profile, mod_ids: [...profile.mod_ids] };
    });
  }

  setActiveProfile(id: string): Promise<void> {
    return this.run('setActiveProfile', [id], () => {
      this.requireProfile(id);
      this.config = { ...this.config, active_profile: id };
    });
  }

  play(profileId: string): Promise<void> {
    return this.run('play', [profileId], () => {
      if (!this.game) {
        throw { code: 'game_not_configured', message: 'no game configured' } satisfies CommandError;
      }
      if (this.game.app_id === null) {
        throw {
          code: 'missing_app_id',
          message: 'game App ID not found; open the game from Steam once',
        } satisfies CommandError;
      }
      if (this.manifestState() === 'stale') {
        throw { code: 'recovery_pending', message: 'a stale session is pending' } satisfies CommandError;
      }
      this.requireProfile(profileId);
    });
  }

  currentSession(): Promise<CurrentSession> {
    return this.run('currentSession', [], () => ({ ...this.current }));
  }

  stopSession(): Promise<void> {
    return this.run('stopSession', [], () => undefined);
  }

  recoveryStatus(): Promise<RecoveryStatus> {
    return this.run('recoveryStatus', [], () => ({ ...this.recovery }));
  }

  deploySession(): Promise<DeploySession> {
    return this.run('deploySession', [], () => {
      const state = this.manifestState();
      return { state, mods_dir: this.manifest?.mods_dir ?? null, tracked: this.manifest?.tracked ?? 0 };
    });
  }

  forgetStaleSession(): Promise<void> {
    return this.run('forgetStaleSession', [], () => {
      this.requireStale();
      this.manifest = null;
    });
  }

  cleanStaleSession(confirmedDir: string): Promise<number> {
    return this.run('cleanStaleSession', [confirmedDir], () => {
      const manifest = this.requireStale();
      if (confirmedDir !== manifest.mods_dir) {
        throw { code: 'deploy_unauthorized_directory', message: 'folder mismatch' } satisfies CommandError;
      }
      this.manifest = null;
      return manifest.tracked;
    });
  }

  getStorageUsage(): Promise<StorageUsage> {
    return this.run('getStorageUsage', [], () => ({
      ...this.storage,
      categories: this.storage.categories.map((entry) => ({ ...entry })),
    }));
  }

  getGameArtwork(): Promise<string | null> {
    return this.run('getGameArtwork', [], () => (this.game ? this.artwork : null));
  }

  // Online catalog (#72): an empty, fresh catalog.
  getCatalog(): Promise<Catalog> {
    return this.run('getCatalog', [], () => ({ entries: [], fetchedAt: 0, stale: false, skipped: 0 }));
  }

  refreshCatalog(): Promise<Catalog> {
    return this.run('refreshCatalog', [], () => ({ entries: [], fetchedAt: 0, stale: false, skipped: 0 }));
  }

  openCatalogPage(id: string): Promise<void> {
    return this.run('openCatalogPage', [id], () => undefined);
  }

  /** Mods the next `installCatalogEntry` imports; defaults to one mod named after the entry. */
  installResult: ModEntry[] | null = null;
  /** When set, `installCatalogEntry` waits for it (to test in-flight behaviour). */
  installGate: Promise<void> | null = null;
  private readonly progressListeners = new Set<(progress: CatalogInstallProgress) => void>();

  async installCatalogEntry(id: string): Promise<ModEntry[]> {
    // The call is recorded (and may fail) first; the gate then keeps it "in flight".
    await this.run('installCatalogEntry', [id], () => undefined);
    if (this.installGate) await this.installGate;
    const mods = this.installResult ?? [{ id, name: id, files: [`${id}.pak`], size_bytes: 1024 }];
    for (const mod of mods) if (!this.mods.some((m) => m.id === mod.id)) this.mods.push(mod);
    return mods.map((mod) => ({ ...mod }));
  }

  cancelCatalogInstall(id: string): Promise<void> {
    return this.run('cancelCatalogInstall', [id], () => undefined);
  }

  onCatalogInstallProgress(handler: (progress: CatalogInstallProgress) => void): Unsubscribe {
    this.progressListeners.add(handler);
    return () => {
      this.progressListeners.delete(handler);
    };
  }

  /** Delivers a progress event to the subscribed controller. */
  emitProgress(progress: CatalogInstallProgress): void {
    for (const listener of [...this.progressListeners]) listener(progress);
  }

  get progressSubscribers(): number {
    return this.progressListeners.size;
  }

  openDataFolder(): Promise<void> {
    return this.run('openDataFolder', [], () => undefined);
  }

  /** Classifies the manifest against the current game, like the backend does. */
  manifestState(): DeploySession['state'] {
    if (!this.manifest) return 'none';
    if (this.manifest.vanished) return 'vanished';
    return this.game && this.manifest.mods_dir === this.game.mods_dir ? 'current' : 'stale';
  }

  private requireStale(): FakeManifest {
    if (this.manifestState() !== 'stale' || !this.manifest) {
      throw { code: 'deploy_session_not_stale', message: 'no stale session' } satisfies CommandError;
    }
    return this.manifest;
  }

  private requireProfile(id: string): Profile {
    const profile = this.profiles.find((candidate) => candidate.id === id);
    if (!profile) throw { code: 'profile_not_found', message: `profile ${id} not found` } satisfies CommandError;
    return profile;
  }

  private buildGame(path: string): GameInfo {
    if (this.options.gameFor) return this.options.gameFor(path);
    const root = path.replace(/[\\/]+$/, '');
    return {
      name: baseName(root),
      root,
      mods_dir: `${root}/${baseName(root)}/Content/Paks/~mods`,
      app_id: null,
      ...this.options.games?.[path],
    };
  }
}

export class FakePicker implements FilePicker {
  readonly modAnswers: Array<string[] | null> = [];
  readonly directoryAnswers: Array<string | null> = [];
  modPicks = 0;
  directoryPicks = 0;

  /** Queues the next answer of `pickModFiles`; `null` cancels the dialog. */
  queueModFiles(answer: string[] | null): void {
    this.modAnswers.push(answer);
  }

  /** Queues the next answer of `pickGameDirectory`; `null` cancels the dialog. */
  queueDirectory(answer: string | null): void {
    this.directoryAnswers.push(answer);
  }

  pickModFiles(): Promise<string[] | null> {
    this.modPicks += 1;
    return Promise.resolve(this.modAnswers.length > 0 ? (this.modAnswers.shift() ?? null) : null);
  }

  pickGameDirectory(): Promise<string | null> {
    this.directoryPicks += 1;
    return Promise.resolve(this.directoryAnswers.length > 0 ? (this.directoryAnswers.shift() ?? null) : null);
  }
}

class Listeners<T> {
  private readonly callbacks = new Set<(event: T) => void>();

  constructor(
    private readonly name: string,
    private readonly log: string[] | undefined,
  ) {}

  add(callback: (event: T) => void): Promise<Unsubscribe> {
    this.log?.push(`subscribe.${this.name}`);
    this.callbacks.add(callback);
    return Promise.resolve(() => {
      this.callbacks.delete(callback);
    });
  }

  emit(event: T): void {
    for (const callback of [...this.callbacks]) callback(event);
  }

  get count(): number {
    return this.callbacks.size;
  }
}

export interface FakeSessionEventsOptions {
  log?: string[];
  /** Rejection reasons: makes the matching subscription reject instead of registering. */
  rejectSession?: unknown;
  rejectSessionError?: unknown;
}

export class FakeSessionEvents implements SessionEvents {
  private readonly session: Listeners<SessionEvent>;
  private readonly errors: Listeners<SessionErrorEvent>;

  constructor(private readonly options: FakeSessionEventsOptions = {}) {
    this.session = new Listeners('session', options.log);
    this.errors = new Listeners('session-error', options.log);
  }

  onSession(callback: (event: SessionEvent) => void): Promise<Unsubscribe> {
    if ('rejectSession' in this.options) return Promise.reject(this.options.rejectSession);
    return this.session.add(callback);
  }

  onSessionError(callback: (event: SessionErrorEvent) => void): Promise<Unsubscribe> {
    if ('rejectSessionError' in this.options) return Promise.reject(this.options.rejectSessionError);
    return this.errors.add(callback);
  }

  emitSession(event: SessionEvent): void {
    this.session.emit(event);
  }

  emitSessionError(message: string): void {
    this.errors.emit({ message });
  }

  /** Number of callbacks still registered (session + session-error). */
  activeListeners(): number {
    return this.session.count + this.errors.count;
  }
}

export class FakeDropEvents implements DropEvents {
  private readonly drops: Listeners<string[]>;

  constructor(options: { log?: string[] } = {}) {
    this.drops = new Listeners('drop', options.log);
  }

  onDrop(callback: (paths: string[]) => void): Promise<Unsubscribe> {
    return this.drops.add(callback);
  }

  emitDrop(paths: string[]): void {
    this.drops.emit(paths);
  }

  activeListeners(): number {
    return this.drops.count;
  }
}
