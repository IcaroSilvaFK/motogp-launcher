import type { UpdateError, UpdateErrorKind, UpdateInfo, UpdateProgress } from '../../domain';
import { isNewerVersion } from '../../domain';
import type { UpdateController, UpdateDeps, UpdateState } from '../updateController';

const KNOWN_KINDS: readonly UpdateErrorKind[] = ['network', 'signature', 'install', 'busy', 'session'];

function toUpdateError(cause: unknown): UpdateError {
  if (typeof cause === 'object' && cause !== null) {
    const { kind, message } = cause as Partial<UpdateError>;
    if (KNOWN_KINDS.includes(kind as UpdateErrorKind) && typeof message === 'string') {
      return { kind: kind as UpdateErrorKind, message };
    }
    if (cause instanceof Error) return { kind: 'unknown', message: cause.message };
  }
  return { kind: 'unknown', message: String(cause) };
}

export function createUpdateController(deps: UpdateDeps): UpdateController {
  let state: UpdateState = { kind: 'idle' };
  const listeners = new Set<() => void>();

  function set(next: UpdateState): void {
    state = next;
    for (const listener of [...listeners]) listener();
  }

  async function check(): Promise<void> {
    if (state.kind === 'downloading' || state.kind === 'installed') return;
    try {
      const info = await deps.service.check();
      if (info !== null && isNewerVersion(info.version, info.currentVersion)) {
        set({ kind: 'available', info });
      } else {
        set({ kind: 'none' });
      }
    } catch (cause) {
      set({ kind: 'error', info: null, error: toUpdateError(cause) });
    }
  }

  function installableInfo(): UpdateInfo | null {
    if (state.kind === 'available' || state.kind === 'error') return state.info;
    return null;
  }

  async function install(): Promise<void> {
    const info = installableInfo();
    if (info === null) return;
    if (!deps.isSessionClean()) {
      set({
        kind: 'error',
        info,
        error: { kind: 'session', message: 'Close the game before updating.' },
      });
      return;
    }
    let downloaded = 0;
    let total: number | null = null;
    set({ kind: 'downloading', info, downloaded, total });
    const onProgress = (event: UpdateProgress): void => {
      if (event.kind === 'started') total = event.total;
      else if (event.kind === 'progress') downloaded += event.chunk;
      else return;
      set({ kind: 'downloading', info, downloaded, total });
    };
    try {
      await deps.service.downloadAndInstall(onProgress);
      set({ kind: 'installed', info });
      await deps.service.restart();
    } catch (cause) {
      set({ kind: 'error', info, error: toUpdateError(cause) });
    }
  }

  return {
    getState: () => state,
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    check,
    install,
  };
}
