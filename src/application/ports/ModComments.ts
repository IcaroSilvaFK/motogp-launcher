import type { CommentsPage, ModComment } from '../../domain';

/**
 * Mod comments on the community server (#118). The backend holds the session token (Windows
 * Credential Manager) and downloads the avatars; the UI never sees either. Every method rejects with
 * a `ModCommentsError` (`offline`, `signed_out`, `rate_limited`, `invalid`, `not_found` or `failed`).
 */
export interface ModComments {
  /** One page, oldest first. Works signed out; pass `nextCursor` of the previous page to continue. */
  list(modId: string, cursor?: string | null): Promise<CommentsPage>;
  /** Needs a session. Resolves with the stored comment. */
  post(modId: string, body: string): Promise<ModComment>;
  /** Author only. Resolves with the updated comment (`edited: true`). */
  edit(modId: string, commentId: string, body: string): Promise<ModComment>;
  /** Author only. */
  remove(modId: string, commentId: string): Promise<void>;
}
