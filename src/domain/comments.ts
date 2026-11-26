// Mod comments (#118). The backend owns the community session token and the avatar downloads; the
// UI only sees these shapes. Bodies are plain text: render them with the safe Markdown renderer.
import { RASTER_DATA_URL } from './community';

/** Longest comment, in characters after trimming (the server enforces the same cap). */
export const MAX_COMMENT_LENGTH = 2000;

/** One comment as the backend returns it. */
export interface ModComment {
  id: string;
  authorName: string;
  /** A raster `data:` URL (the CSP only allows `data:` images), or `null` to show initials. */
  authorAvatarDataUrl: string | null;
  /** Plain text, possibly with Markdown; never HTML. */
  body: string;
  /** Creation time in milliseconds since the Unix epoch. */
  createdAtMs: number;
  edited: boolean;
  /** Written by the signed-in member: only these can be edited or deleted. */
  own: boolean;
}

/** One page of comments, oldest first. */
export interface CommentsPage {
  comments: ModComment[];
  /** Pass it back to read the next page; `null` when this was the last one. */
  nextCursor: string | null;
}

/**
 * Why a comment operation failed. `offline`: the server could not be reached; `signed_out`: a write
 * needs a session (the stored one is already forgotten when the server rejected it); `rate_limited`:
 * too many writes, see `retryAfterSecs`; `invalid`: empty, too long or otherwise refused; `not_found`:
 * the comment is gone; `banned`: the community account is suspended from writes (the session stays valid);
 * `failed`: anything else.
 */
export type ModCommentsErrorCode =
  'offline' | 'signed_out' | 'rate_limited' | 'invalid' | 'not_found' | 'banned' | 'failed';

/** Rejection of every `ModComments` call, carrying a typed code the UI can translate. */
export class ModCommentsError extends Error {
  readonly code: ModCommentsErrorCode;
  /** Seconds to wait before writing again; set only for `rate_limited`. */
  readonly retryAfterSecs: number | null;

  constructor(code: ModCommentsErrorCode, retryAfterSecs: number | null = null) {
    super(`mod comments failed: ${code}`);
    this.name = 'ModCommentsError';
    this.code = code;
    this.retryAfterSecs = retryAfterSecs;
  }
}

const BACKEND_CODES: Record<string, ModCommentsErrorCode> = {
  community_offline: 'offline',
  community_signed_out: 'signed_out',
  comment_rate_limited: 'rate_limited',
  comment_invalid: 'invalid',
  comment_not_found: 'not_found',
  community_banned: 'banned',
};

// The backend writes "... Try again in <N> seconds." into the message of `comment_rate_limited`.
const RETRY_SECONDS = /(\d+) seconds?\.?\s*$/;

/** Maps a rejected IPC error (`{ code, message }`) to a typed error; anything unknown is `failed`. */
export function toModCommentsError(error: unknown): ModCommentsError {
  if (error instanceof ModCommentsError) return error;
  if (typeof error !== 'object' || error === null) return new ModCommentsError('failed');
  const { code, message } = error as { code?: unknown; message?: unknown };
  const mapped = (typeof code === 'string' && BACKEND_CODES[code]) || 'failed';
  if (mapped !== 'rate_limited') return new ModCommentsError(mapped);
  const seconds = typeof message === 'string' ? RETRY_SECONDS.exec(message)?.[1] : undefined;
  return new ModCommentsError('rate_limited', seconds === undefined ? 1 : Math.max(1, Number(seconds)));
}

/** Validates one untrusted comment; `null` when it is malformed. */
export function parseModComment(value: unknown): ModComment | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as Record<string, unknown>;
  if (typeof v.id !== 'string' || v.id === '') return null;
  if (typeof v.authorName !== 'string' || typeof v.body !== 'string') return null;
  if (typeof v.createdAtMs !== 'number' || !Number.isFinite(v.createdAtMs)) return null;
  if (typeof v.edited !== 'boolean' || typeof v.own !== 'boolean') return null;
  const avatar =
    typeof v.authorAvatarDataUrl === 'string' && RASTER_DATA_URL.test(v.authorAvatarDataUrl)
      ? v.authorAvatarDataUrl
      : null;
  return {
    id: v.id,
    authorName: v.authorName,
    authorAvatarDataUrl: avatar,
    body: v.body,
    createdAtMs: v.createdAtMs,
    edited: v.edited,
    own: v.own,
  };
}

/** Validates an untrusted page; `null` when the answer is not a page (a malformed comment fails it). */
export function parseCommentsPage(value: unknown): CommentsPage | null {
  if (typeof value !== 'object' || value === null) return null;
  const { comments, nextCursor } = value as { comments?: unknown; nextCursor?: unknown };
  if (!Array.isArray(comments)) return null;
  if (nextCursor !== null && nextCursor !== undefined && typeof nextCursor !== 'string') return null;
  const parsed: ModComment[] = [];
  for (const item of comments) {
    const comment = parseModComment(item);
    if (!comment) return null;
    parsed.push(comment);
  }
  return { comments: parsed, nextCursor: nextCursor ?? null };
}

/** Why a draft cannot be sent, or `null` when it can. Same rules as the server (after trimming). */
export function commentDraftProblem(text: string): 'empty' | 'too_long' | null {
  const trimmed = text.replace(/\r\n/g, '\n').trim();
  if (trimmed === '') return 'empty';
  return [...trimmed].length > MAX_COMMENT_LENGTH ? 'too_long' : null;
}

/** Why a member reports a comment (the server's wire values). */
export type ReportReason = 'spam' | 'abuse' | 'off_topic' | 'other';

/** Every report reason, in the order a picker shows them. */
export const REPORT_REASONS: readonly ReportReason[] = ['spam', 'abuse', 'off_topic', 'other'];

/** Longest optional detail text of a report, in characters (the server enforces the same cap). */
export const MAX_REPORT_DETAILS_LENGTH = 500;
