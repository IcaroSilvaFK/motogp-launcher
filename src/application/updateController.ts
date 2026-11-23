import type { UpdateError, UpdateInfo } from '../domain';
import type { UpdateService } from './ports/UpdateService';

export type UpdateState =
  | { kind: 'idle' }
  | { kind: 'none' }
  | { kind: 'available'; info: UpdateInfo }
  | { kind: 'downloading'; info: UpdateInfo; downloaded: number; total: number | null }
  | { kind: 'installed'; info: UpdateInfo }
  | { kind: 'error'; info: UpdateInfo | null; error: UpdateError };

export interface UpdateController {
  getState(): UpdateState;
  /** useSyncExternalStore-compatible. */
  subscribe(listener: () => void): () => void;
  /** Never throws. */
  check(): Promise<void>;
  /** Never throws; no-op while one is in progress. */
  install(): Promise<void>;
}

export interface UpdateDeps {
  service: UpdateService;
  isSessionClean: () => boolean;
}
