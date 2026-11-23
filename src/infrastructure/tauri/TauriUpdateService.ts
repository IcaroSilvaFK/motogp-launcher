import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import type { UpdateError, UpdateErrorKind, UpdateInfo, UpdateProgress } from '../../domain';
import type { UpdateService } from '../../application/ports/UpdateService';

/*
 * The updater plugin rejects with plain strings or Errors, so the kind is a
 * message heuristic: "signature"/"verify" -> 'signature'; network-like words
 * (request, connection, timeout, dns, status, network) -> 'network'; the rest
 * is 'network' during check (nothing was touched locally) and 'install' during
 * download/install. The original message is always kept.
 */
const SIGNATURE_PATTERN = /signature|verif/i;
const NETWORK_PATTERN = /network|request|connect|timeout|timed out|dns|status|offline|resolve/i;

function messageOf(error: unknown): string {
  if (error instanceof Error) return error.message;
  return typeof error === 'string' ? error : String(error);
}

function toUpdateError(error: unknown, stage: 'check' | 'install'): UpdateError {
  const message = messageOf(error);
  let kind: UpdateErrorKind;
  if (SIGNATURE_PATTERN.test(message)) kind = 'signature';
  else if (stage === 'check' || NETWORK_PATTERN.test(message)) kind = 'network';
  else kind = 'install';
  return { kind, message };
}

export function createTauriUpdateService(): UpdateService {
  let pending: Update | null = null;

  return {
    async check(): Promise<UpdateInfo | null> {
      pending = null;
      let update: Update | null;
      try {
        update = await check();
      } catch (error) {
        throw toUpdateError(error, 'check');
      }
      if (!update) return null;
      pending = update;
      return {
        version: update.version,
        currentVersion: update.currentVersion,
        notes: update.body ?? null,
        date: update.date ?? null,
      };
    },

    async downloadAndInstall(onProgress: (event: UpdateProgress) => void): Promise<void> {
      if (!pending) {
        throw { kind: 'unknown', message: 'No update available to install' } satisfies UpdateError;
      }
      try {
        await pending.downloadAndInstall((event) => {
          if (event.event === 'Started') {
            onProgress({ kind: 'started', total: event.data.contentLength ?? null });
          } else if (event.event === 'Progress') {
            onProgress({ kind: 'progress', chunk: event.data.chunkLength });
          } else if (event.event === 'Finished') {
            onProgress({ kind: 'finished' });
          }
        });
      } catch (error) {
        throw toUpdateError(error, 'install');
      }
    },

    async restart(): Promise<void> {
      await relaunch();
    },
  };
}
