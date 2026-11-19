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

export type SessionState = 'started' | 'ended' | 'never-started';

export interface SessionEvent {
  state: SessionState;
  cleaned: number;
}

export interface SessionErrorEvent {
  message: string;
}

export type RecoveryState = 'not-run' | 'recovered' | 'game-running' | 'failed';

export interface RecoveryStatus {
  state: RecoveryState;
  error: string | null;
  cleaned_count: number;
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
