import type { CommunityStatus } from '../../domain';

/**
 * The community account (#122). The backend holds the session token (Windows Credential Manager)
 * and talks to the community server; the UI only sees the status.
 */
export interface CommunityAccount {
  /** Never rejects: a missing command, a malformed answer or an unreachable server is `offline`. */
  status(): Promise<CommunityStatus>;
  /**
   * Opens the browser for Discord and resolves with the new status when the user comes back.
   * Rejects with a `CommunitySignInError` (`offline`, `failed`, `timeout` or `denied`).
   */
  signIn(): Promise<CommunityStatus>;
  /** Revokes the session when possible and always forgets it locally. Never rejects. */
  signOut(): Promise<void>;
}
