// In-memory mod hosting ports for the publish UI tests (#130). They behave like the backend: one
// publish at a time, progress events while uploading, a typed refusal for a version that already
// exists, cancel answering `cancelled`, and "My mods" reading what was stored.
import { vi } from 'vitest';
import type { ModPublisher, PublishFilePicker } from '../../application';
import { PublishError } from '../../domain';
import type { ModVersion, MyMod, PublishErrorCode, PublishProgress, PublishRequest } from '../../domain';

export const FILE_SIZE = 2 * 1024 * 1024;

export function version(patch: Partial<ModVersion> & { modId: string; version: string }): ModVersion {
  return {
    title: 'Brazil Livery',
    description: 'Green and yellow.',
    category: 'liveries',
    game: 'MotoGP 26',
    fileName: 'brazil-livery.zip',
    size: FILE_SIZE,
    sha256: 'a'.repeat(64),
    status: 'pending',
    screenshotCount: 0,
    uploadedAt: '2026-11-20T12:00:00Z',
    ...patch,
  };
}

export class FakeModPublisher implements ModPublisher {
  readonly stored = new Map<string, ModVersion[]>();
  private listeners = new Set<(progress: PublishProgress) => void>();
  /** When set, `publish` waits here until the test calls `finish` (or the user cancels). */
  private holding = false;
  private pending: { resolve: () => void; reject: (error: unknown) => void } | null = null;
  offline = false;

  readonly publish = vi.fn((request: PublishRequest): Promise<ModVersion> => {
    const versions = this.stored.get(request.modId) ?? [];
    if (versions.some((v) => v.version === request.version)) {
      return Promise.reject(new PublishError('version_exists', 'version exists'));
    }
    const fileName = request.file.split(/[\\/]/).pop() ?? request.file;
    const stored = version({
      modId: request.modId,
      version: request.version,
      title: request.title,
      description: request.description,
      category: request.category,
      game: request.game,
      fileName,
      screenshotCount: request.screenshots.length,
    });
    const store = () => {
      this.stored.set(request.modId, [...versions, stored]);
      return stored;
    };
    if (!this.holding) return Promise.resolve(store());
    return new Promise<ModVersion>((resolve, reject) => {
      this.pending = { resolve: () => resolve(store()), reject };
    });
  });

  readonly cancel = vi.fn((): Promise<void> => {
    const pending = this.pending;
    this.pending = null;
    pending?.reject(new PublishError('cancelled'));
    return Promise.resolve();
  });

  readonly myMods = vi.fn((): Promise<MyMod[]> => {
    if (this.offline) return Promise.reject(new PublishError('offline'));
    return Promise.resolve([...this.stored].map(([id, versions]) => ({ id, versions })));
  });

  onProgress(callback: (progress: PublishProgress) => void) {
    this.listeners.add(callback);
    return () => {
      this.listeners.delete(callback);
    };
  }

  hold(): void {
    this.holding = true;
  }

  emit(progress: PublishProgress): void {
    for (const listener of this.listeners) listener(progress);
  }

  finish(): void {
    const pending = this.pending;
    this.pending = null;
    pending?.resolve();
  }

  fail(code: PublishErrorCode): void {
    const pending = this.pending;
    this.pending = null;
    pending?.reject(new PublishError(code));
  }
}

/** Native dialogs answered by the test: the next file and the next screenshots to return. */
export class FakePublishPicker implements PublishFilePicker {
  nextFile: string | null = 'C:\\Mods\\brazil-livery.zip';
  nextScreenshots: string[] | null = [];

  readonly pickModFile = vi.fn(() => Promise.resolve(this.nextFile));
  readonly pickScreenshots = vi.fn(() => Promise.resolve(this.nextScreenshots));
}
