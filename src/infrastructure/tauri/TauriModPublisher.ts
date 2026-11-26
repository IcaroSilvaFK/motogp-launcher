import { invoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';
import type { ModPublisher } from '../../application';
import { PublishError, parseModVersion, parseMyMods, parsePublishProgress, toPublishError } from '../../domain';
import type { PublishProgress } from '../../domain';
import { subscribe } from '../subscription';
import type { ListenFn } from '../tauriSessionEvents';

async function call(command: string, args?: Record<string, unknown>): Promise<unknown> {
  try {
    return await invoke(command, args);
  } catch (error) {
    throw toPublishError(error);
  }
}

/**
 * Mod publishing over IPC (`community_publish_mod`, `community_cancel_publish`, `community_my_mods`
 * and the `community-publish-progress` event). Rejections are typed `PublishError`s; a malformed
 * answer is `server_error`.
 */
export function createTauriModPublisher(listen: ListenFn = tauriListen): ModPublisher {
  return {
    async publish(request) {
      const version = parseModVersion(await call('community_publish_mod', { request }));
      if (!version) throw new PublishError('server_error', 'malformed publish answer');
      return version;
    },
    async cancel() {
      await call('community_cancel_publish');
    },
    async myMods() {
      const mods = parseMyMods(await call('community_my_mods'));
      if (!mods) throw new PublishError('server_error', 'malformed my-mods answer');
      return mods;
    },
    onProgress(callback) {
      return subscribe<PublishProgress>(
        (deliver) =>
          listen<unknown>('community-publish-progress', (event) => {
            const progress = parsePublishProgress(event.payload);
            if (progress) deliver(progress);
          }),
        callback,
      );
    },
  };
}
