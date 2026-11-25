// Domain types mirroring the backend IPC (issue 08). Field names stay snake_case.

export const VANILLA_ID = 'vanilla';

export interface Config {
  game_dir: string | null;
  active_profile: string | null;
}

export interface GameInfo {
  name: string;
  root: string;
  mods_dir: string;
  app_id: number | null;
}

/** Files that several mods of a load order ship; `mod_ids` are in load order and the last one wins. */
export interface FileConflict {
  path: string;
  mod_ids: string[];
  winner: string;
}

export interface ModEntry {
  id: string;
  name: string;
  files: string[];
  size_bytes: number;
}

export interface Profile {
  id: string;
  name: string;
  mod_ids: string[];
}

export type SessionState = 'started' | 'cleaning-up' | 'ended' | 'never-started';

export interface SessionEvent {
  state: SessionState;
  cleaned: number;
  /** Unix ms when the game was first seen; `null` while it never was. */
  started_at: number | null;
}

export type SessionPhaseName = 'idle' | 'starting' | 'running' | 'stopping' | 'cleaning-up';

/** The live session as the backend sees it (`current_session`). */
export interface CurrentSession {
  phase: SessionPhaseName;
  /** Unix ms when the game was first seen; stays set while stopping and cleaning up. */
  started_at: number | null;
}

export interface SessionErrorEvent {
  message: string;
}

export type DeploySessionState = 'none' | 'current' | 'stale' | 'vanished';

export interface DeploySession {
  state: DeploySessionState;
  mods_dir: string | null;
  tracked: number;
}

export type RecoveryState = 'not-run' | 'recovered' | 'game-running' | 'failed';

export interface RecoveryStatus {
  state: RecoveryState;
  error: string | null;
  cleaned_count: number;
}

/** Storage categories of the launcher data folder, in their fixed display order. */
export const STORAGE_CATEGORIES = ['mods', 'launcher_data', 'other'] as const;

export type StorageCategory = (typeof STORAGE_CATEGORIES)[number];

/** Disk usage of the launcher data folder (camelCase: mapped from the backend DTO by the gateway). */
export interface StorageUsage {
  root: string;
  totalBytes: number;
  /** `null` when the free space of the drive could not be read. */
  freeBytes: number | null;
  categories: { category: StorageCategory; bytes: number }[];
}

export const BACKEND_ERROR_CODES = [
  'read_failed',
  'write_failed',
  'data_corrupted',
  'task_failed',
  'not_unreal_game',
  'mod_without_pak',
  'unsupported_format',
  'no_pak_found',
  'name_collision',
  'invalid_mod_id',
  'mod_not_found',
  'archive_too_large',
  'empty_profile_name',
  'profile_not_found',
  'vanilla_protected',
  'profile_locked_by_session',
  'library_locked_by_session',
  'already_running',
  'game_not_configured',
  'missing_app_id',
  'open_game_failed',
  'recovery_pending',
  'game_still_running',
  'process_inspection_failed',
  'session_thread_failed',
  'deploy_storage_failed',
  'deploy_io_failed',
  'deploy_already_exists',
  'deploy_invalid_path',
  'deploy_unauthorized_directory',
  'deploy_cleanup_incomplete',
  'deploy_session_not_stale',
  'no_active_session',
  'stop_failed',
  // Online catalog (#72).
  'catalog_unavailable',
  'catalog_unsupported_schema',
  // Opening a catalog page (#73).
  'catalog_entry_not_found',
  'catalog_page_not_allowed',
  'open_page_failed',
  // Catalog downloads (#74).
  'catalog_entry_not_downloadable',
  'catalog_install_in_progress',
  'download_network',
  'download_size_mismatch',
  'download_hash_mismatch',
  'download_cancelled',
] as const;

export const FRONTEND_ERROR_CODES = [
  'session_error',
  'session_subscription_failed',
  'recovery_failed',
  'unknown',
] as const;

export const ERROR_CODES = [...BACKEND_ERROR_CODES, ...FRONTEND_ERROR_CODES] as const;

export type ErrorCode = (typeof ERROR_CODES)[number];

export interface CommandError {
  code: ErrorCode;
  message: string;
}

// ---- Online catalog (#72): camelCase, mapped 1:1 from the backend DTO. ----

/** A file the catalog author allows the launcher to download itself (#74 verifies size and hash). */
export interface CatalogDownload {
  url: string;
  size: number;
  /** 64 lowercase hex characters. */
  sha256: string;
}

export interface CatalogEntry {
  id: string;
  title: string;
  author: string;
  game: string;
  category: string;
  description: string;
  version: string;
  /** `YYYY-MM-DD`. */
  updatedAt: string;
  /** https page where the mod lives. */
  pageUrl: string;
  thumbnailUrl: string | null;
  /** Present only when the author allows redistribution. */
  download: CatalogDownload | null;
}

export interface Catalog {
  entries: CatalogEntry[];
  /** Unix milliseconds of the last successful fetch. */
  fetchedAt: number;
  /** `true` when the source was unreachable and this is an older cached copy. */
  stale: boolean;
  /** Catalog entries the launcher dropped as invalid or duplicate. */
  skipped: number;
}

// ---- Catalog installs (#74). ----

export type CatalogInstallPhase = 'downloading' | 'verifying' | 'importing';

/** Payload of the `catalog-install-progress` event (camelCase, as emitted by the backend). */
export interface CatalogInstallProgress {
  id: string;
  phase: CatalogInstallPhase;
  /** Bytes received so far. */
  received: number;
  /** Bytes expected in total. */
  total: number;
}

/** Install state of one catalog entry. `percent` is an integer 0-100. */
export type CatalogInstallState =
  | { status: 'idle' }
  | { status: 'downloading'; received: number; total: number; percent: number }
  | { status: 'verifying' }
  | { status: 'importing' }
  | { status: 'installed' }
  | { status: 'failed'; code: ErrorCode; message: string };
