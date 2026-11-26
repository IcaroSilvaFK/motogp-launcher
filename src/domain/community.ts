// Community account (#122). The backend owns the Discord sign-in, the session token and the avatar
// download; the UI only ever sees this state. The token never crosses IPC.

/** The public profile of the signed-in member. */
export interface CommunityProfile {
  displayName: string;
  /** A raster `data:` URL (the CSP only allows `data:` images), or `null` to show initials. */
  avatarDataUrl: string | null;
}

/** Where the community sign-in stands, as `community_me` / `community_sign_in` answer. */
export type CommunityStatus =
  { state: 'signed_out' } | { state: 'offline' } | { state: 'signed_in'; profile: CommunityProfile };

/** Why a sign-in did not finish. `offline` means the server could not be reached. */
export type CommunityErrorCode = 'offline' | 'failed' | 'timeout' | 'denied';

/** Rejection of `CommunityAccount.signIn`, carrying a typed code the UI can translate. */
export class CommunitySignInError extends Error {
  readonly code: CommunityErrorCode;

  constructor(code: CommunityErrorCode) {
    super(`community sign-in failed: ${code}`);
    this.name = 'CommunitySignInError';
    this.code = code;
  }
}

const BACKEND_CODES: Record<string, CommunityErrorCode> = {
  community_offline: 'offline',
  community_sign_in_failed: 'failed',
  community_sign_in_timeout: 'timeout',
  community_sign_in_denied: 'denied',
};

/** Maps a rejected IPC error (`{ code, message }`) to a typed error; anything unknown is `failed`. */
export function toCommunitySignInError(error: unknown): CommunitySignInError {
  if (error instanceof CommunitySignInError) return error;
  const code = typeof error === 'object' && error !== null ? (error as { code?: unknown }).code : undefined;
  return new CommunitySignInError((typeof code === 'string' && BACKEND_CODES[code]) || 'failed');
}

/** Raster images only: no SVG, which could carry script. */
export const RASTER_DATA_URL = /^data:image\/(png|jpeg|webp|gif);base64,[A-Za-z0-9+/]+=*$/;

/**
 * Validates an untrusted IPC answer. `null` for anything that is not a known state (a missing
 * command, a malformed answer); callers decide what that means (`offline` for status reads).
 */
export function parseCommunityStatus(value: unknown): CommunityStatus | null {
  if (typeof value !== 'object' || value === null) return null;
  const { state, profile } = value as { state?: unknown; profile?: unknown };
  if (state === 'signed_out' || state === 'offline') return { state };
  if (state !== 'signed_in' || typeof profile !== 'object' || profile === null) return null;
  const { displayName, avatarDataUrl } = profile as { displayName?: unknown; avatarDataUrl?: unknown };
  if (typeof displayName !== 'string' || displayName.trim() === '') return null;
  const avatar = typeof avatarDataUrl === 'string' && RASTER_DATA_URL.test(avatarDataUrl) ? avatarDataUrl : null;
  return { state: 'signed_in', profile: { displayName, avatarDataUrl: avatar } };
}
