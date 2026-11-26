import { invoke } from '@tauri-apps/api/core';
import type { CommunityAccount } from '../../application';
import { CommunitySignInError, parseCommunityStatus, toCommunitySignInError } from '../../domain';

/**
 * The community account over IPC (`community_me`, `community_sign_in`, `community_sign_out`).
 * `status` and `signOut` never reject (a missing command or a malformed answer is `offline`);
 * `signIn` rejects with a typed `CommunitySignInError`.
 */
export function createTauriCommunityAccount(): CommunityAccount {
  return {
    async status() {
      try {
        return parseCommunityStatus(await invoke('community_me')) ?? { state: 'offline' };
      } catch (error) {
        // Expected until the backend ships the command, and while the server is down.
        console.warn('community_me failed', error);
        return { state: 'offline' };
      }
    },
    async signIn() {
      let answer: unknown;
      try {
        answer = await invoke('community_sign_in');
      } catch (error) {
        throw toCommunitySignInError(error);
      }
      const status = parseCommunityStatus(answer);
      // A sign-in that did not end signed in is a failure, whatever the backend said.
      if (status?.state !== 'signed_in') throw new CommunitySignInError('failed');
      return status;
    },
    async signOut() {
      try {
        await invoke('community_sign_out');
      } catch (error) {
        console.warn('community_sign_out failed', error);
      }
    },
  };
}
