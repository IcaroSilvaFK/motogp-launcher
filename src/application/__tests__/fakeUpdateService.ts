import type { UpdateInfo, UpdateProgress } from '../../domain';
import type { UpdateService } from '../ports/UpdateService';

/** In-memory UpdateService double. Test-only code. */
export class FakeUpdateService implements UpdateService {
  /** Result of `check()`; null = no update. */
  checkResult: UpdateInfo | null = null;
  /** When set, `check()` rejects with it. */
  checkError: unknown = null;
  /** When set, `downloadAndInstall()` rejects with it (after emitting progress). */
  installError: unknown = null;
  /** When set, `restart()` rejects with it. */
  restartError: unknown = null;
  /** Events emitted to the progress callback, in order, during `downloadAndInstall()`. */
  progressEvents: UpdateProgress[] = [];

  checkCalls = 0;
  downloadCalls = 0;
  restartCalls = 0;

  private gate: Promise<void> | null = null;
  private release: (() => void) | null = null;

  /** Makes the next download wait (after emitting progress) until `releaseDownload()`. */
  holdDownload(): void {
    this.gate = new Promise<void>((resolve) => {
      this.release = resolve;
    });
  }

  releaseDownload(): void {
    this.release?.();
    this.gate = null;
    this.release = null;
  }

  check(): Promise<UpdateInfo | null> {
    this.checkCalls += 1;
    if (this.checkError !== null) return Promise.reject(this.checkError);
    return Promise.resolve(this.checkResult);
  }

  async downloadAndInstall(onProgress: (event: UpdateProgress) => void): Promise<void> {
    this.downloadCalls += 1;
    for (const event of this.progressEvents) onProgress(event);
    if (this.gate) await this.gate;
    if (this.installError !== null) throw this.installError;
  }

  restart(): Promise<void> {
    this.restartCalls += 1;
    if (this.restartError !== null) return Promise.reject(this.restartError);
    return Promise.resolve();
  }
}
