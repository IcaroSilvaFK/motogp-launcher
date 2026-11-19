import type {
  CommandError,
  Config,
  GameInfo,
  ModEntry,
  Profile,
  RecoveryStatus,
  SessionErrorEvent,
  SessionEvent,
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

export interface FakeGatewayOptions {
  config?: Config;
  game?: GameInfo | null;
  mods?: ModEntry[];
  profiles?: Profile[];
  detected?: GameInfo[];
  recovery?: RecoveryStatus;
  /** Overrides per path of what `setGameDir` returns (partial GameInfo); e.g. `{ 'C:/g': { app_id: 123 } }`. */
  games?: Record<string, Partial<GameInfo>>;
  /** Or a callback building the GameInfo for any path. Wins over `games`. */
  gameFor?: (path: string) => GameInfo;
  /** Shared log of ordered events (`gateway.<method>`) to assert subscription order across fakes. */
  log?: string[];
}

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
    this.recovery = options.recovery ?? { state: 'not-run', error: null, cleaned_count: 0 };
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
      this.requireProfile(profileId);
    });
  }

  recoveryStatus(): Promise<RecoveryStatus> {
    return this.run('recoveryStatus', [], () => ({ ...this.recovery }));
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

export class FakeSessionEvents implements SessionEvents {
  private readonly session: Listeners<SessionEvent>;
  private readonly errors: Listeners<SessionErrorEvent>;

  constructor(options: { log?: string[] } = {}) {
    this.session = new Listeners('session', options.log);
    this.errors = new Listeners('session-error', options.log);
  }

  onSession(callback: (event: SessionEvent) => void): Promise<Unsubscribe> {
    return this.session.add(callback);
  }

  onSessionError(callback: (event: SessionErrorEvent) => void): Promise<Unsubscribe> {
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
