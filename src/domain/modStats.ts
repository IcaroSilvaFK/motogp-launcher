// Community numbers of a mod (#119). The backend owns the session token; the UI only sees these.

/** The community numbers of one mod, keyed by its catalog id (slug). */
export interface ModStats {
  id: string;
  stars: number;
  /** Present only when the request carried a valid session. */
  starredByMe?: boolean;
}

/** Why a star or unstar did not go through (`banned`: the community account is suspended). */
export type ModStatsErrorKind = 'signedOut' | 'offline' | 'banned' | 'failed';

/** Rejection of `ModStatsSource.star` / `unstar`, carrying a typed kind the UI can act on. */
export class ModStatsError extends Error {
  readonly kind: ModStatsErrorKind;

  constructor(kind: ModStatsErrorKind) {
    super(`mod stats request failed: ${kind}`);
    this.name = 'ModStatsError';
    this.kind = kind;
  }
}

const BACKEND_KINDS: Record<string, ModStatsErrorKind> = {
  community_signed_out: 'signedOut',
  community_offline: 'offline',
  community_banned: 'banned',
};

/** Maps a rejected IPC error (`{ code, message }`) to a typed error; anything unknown is `failed`. */
export function toModStatsError(error: unknown): ModStatsError {
  if (error instanceof ModStatsError) return error;
  const code = typeof error === 'object' && error !== null ? (error as { code?: unknown }).code : undefined;
  return new ModStatsError((typeof code === 'string' && BACKEND_KINDS[code]) || 'failed');
}

/** Validates one untrusted IPC item; `null` when it is not a mod's stats. */
export function parseModStats(value: unknown): ModStats | null {
  if (typeof value !== 'object' || value === null) return null;
  const { id, stars, starredByMe } = value as { id?: unknown; stars?: unknown; starredByMe?: unknown };
  if (typeof id !== 'string' || id === '') return null;
  if (typeof stars !== 'number' || !Number.isInteger(stars) || stars < 0) return null;
  return typeof starredByMe === 'boolean' ? { id, stars, starredByMe } : { id, stars };
}
