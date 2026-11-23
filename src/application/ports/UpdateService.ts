import type { UpdateInfo, UpdateProgress } from '../../domain';

export interface UpdateService {
  /** null = no newer version (updater semantics). Rejects with UpdateError. */
  check(): Promise<UpdateInfo | null>;
  /** Downloads + installs the update returned by the last check. Rejects with UpdateError. */
  downloadAndInstall(onProgress: (event: UpdateProgress) => void): Promise<void>;
  restart(): Promise<void>;
}
