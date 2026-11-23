import type { UpdateError, UpdateInfo } from '../domain';
import type { UpdateService } from './ports/UpdateService';

export type UpdateState =
  | { kind: 'idle' }
  | { kind: 'none' }
  | { kind: 'available'; info: UpdateInfo }
  | { kind: 'downloading'; info: UpdateInfo; downloaded: number; total: number | null }
  | { kind: 'installed'; info: UpdateInfo }
  | { kind: 'error'; info: UpdateInfo | null; error: UpdateError };

export interface CheckOptions {
  /** True for the silent startup check; false (default) for a user-initiated one. */
  background?: boolean;
}

export interface UpdateController {
  getState(): UpdateState;
  /** useSyncExternalStore-compatible. */
  subscribe(listener: () => void): () => void;
  /**
   * Never throws. By default the check is user-initiated: a failure enters the `error` state.
   * With `{ background: true }` (the startup check) a failure is only logged with `console.warn`:
   * `idle` becomes `none` and any other state is left unchanged, so the UI stays silent.
   */
  check(options?: CheckOptions): Promise<void>;
  /** Never throws; no-op while one is in progress. */
  install(): Promise<void>;
}

export interface UpdateDeps {
  service: UpdateService;
  isSessionClean: () => boolean;
}
