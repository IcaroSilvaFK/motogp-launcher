// In-memory community ports for the mod comments tests (#118): they behave like the backend
// (pages of 20, oldest first, author-only writes, a per-user write limit) without any network.
import { vi } from 'vitest';
import type { CommunityAccount, ModComments } from '../../application';
import { MAX_COMMENT_LENGTH, ModCommentsError } from '../../domain';
import type { CommentsPage, CommunityStatus, ModComment, ReportReason } from '../../domain';

export const PAGE_SIZE = 20;
export const ME = 'rider';
const HOUR = 60 * 60 * 1000;

/** A stored comment: who wrote it decides `own` per caller, like the server does. */
interface Stored {
  id: string;
  author: string;
  avatar: string | null;
  body: string;
  createdAtMs: number;
  edited: boolean;
}

export class FakeModComments implements ModComments {
  private stored: Stored[] = [];
  private nextId = 1;
  /** Writes allowed before the server answers `rate_limited`. */
  writeLimit = Infinity;
  private writes = 0;
  offline = false;
  /** Pending `list` answers held by the test (`holdLists`), released with `releaseLists`. */
  private held: (() => void)[] | null = null;

  constructor(private readonly account: FakeCommunityAccount) {}

  readonly list = vi.fn((_modId: string, cursor?: string | null): Promise<CommentsPage> => {
    const answer = () => {
      if (this.offline) throw new ModCommentsError('offline');
      const start = cursor ? Number(cursor) : 0;
      const slice = this.stored.slice(start, start + PAGE_SIZE);
      const next = start + PAGE_SIZE < this.stored.length ? String(start + PAGE_SIZE) : null;
      return { comments: slice.map((s) => this.view(s)), nextCursor: next };
    };
    if (this.held) {
      const held = this.held;
      return new Promise((resolve, reject) =>
        held.push(() => {
          try {
            resolve(answer());
          } catch (error) {
            reject(error);
          }
        }),
      );
    }
    try {
      return Promise.resolve(answer());
    } catch (error) {
      return Promise.reject(error);
    }
  });

  readonly post = vi.fn((_modId: string, body: string): Promise<ModComment> => {
    const refused = this.refuseWrite(body);
    if (refused) return Promise.reject(refused);
    const comment = this.add(ME, body.trim(), Date.now());
    return Promise.resolve(this.view(comment));
  });

  readonly edit = vi.fn((_modId: string, commentId: string, body: string): Promise<ModComment> => {
    const refused = this.refuseWrite(body);
    if (refused) return Promise.reject(refused);
    const comment = this.stored.find((c) => c.id === commentId);
    // Someone else's comment answers 404, exactly like a missing one.
    if (!comment || comment.author !== this.account.name()) return Promise.reject(new ModCommentsError('not_found'));
    comment.body = body.trim();
    comment.edited = true;
    return Promise.resolve(this.view(comment));
  });

  readonly remove = vi.fn((_modId: string, commentId: string): Promise<void> => {
    const refused = this.refuseWrite(null);
    if (refused) return Promise.reject(refused);
    const comment = this.stored.find((c) => c.id === commentId);
    if (!comment || comment.author !== this.account.name()) return Promise.reject(new ModCommentsError('not_found'));
    this.stored = this.stored.filter((c) => c !== comment);
    return Promise.resolve();
  });

  /** Reports recorded by the fake server: one per (reporter, comment). */
  readonly reports: { commentId: string; reason: ReportReason; details: string | null }[] = [];
  /** A banned account: every write (post, edit, remove, report) answers `banned`. */
  banned = false;

  readonly report = vi.fn(
    (_modId: string, commentId: string, reason: ReportReason, details?: string): Promise<void> => {
      const refused = this.refuseWrite(null);
      if (refused) return Promise.reject(refused);
      const comment = this.stored.find((c) => c.id === commentId);
      // An unknown comment and your own comment are both refused, like the server does.
      if (!comment) return Promise.reject(new ModCommentsError('not_found'));
      if (comment.author === this.account.name()) return Promise.reject(new ModCommentsError('invalid'));
      if (!this.reports.some((r) => r.commentId === commentId)) {
        this.reports.push({ commentId, reason, details: details?.trim() || null });
      }
      return Promise.resolve();
    },
  );

  /** Seeds a comment as if it were stored on the server earlier. */
  add(author: string, body: string, createdAtMs = Date.now() - 2 * HOUR, avatar: string | null = null): Stored {
    const comment: Stored = { id: `c${this.nextId++}`, author, avatar, body, createdAtMs, edited: false };
    this.stored.push(comment);
    return comment;
  }

  bodies(): string[] {
    return this.stored.map((c) => c.body);
  }

  holdLists(): void {
    this.held = [];
  }

  releaseLists(): void {
    const held = this.held ?? [];
    this.held = null;
    for (const release of held) release();
  }

  private refuseWrite(body: string | null): ModCommentsError | null {
    if (this.offline) return new ModCommentsError('offline');
    if (this.account.name() === null) return new ModCommentsError('signed_out');
    if (this.banned) return new ModCommentsError('banned');
    if (this.writes >= this.writeLimit) return new ModCommentsError('rate_limited', 42);
    if (body !== null) {
      const trimmed = body.trim();
      if (trimmed === '' || [...trimmed].length > MAX_COMMENT_LENGTH) return new ModCommentsError('invalid');
    }
    this.writes += 1;
    return null;
  }

  private view(s: Stored): ModComment {
    return {
      id: s.id,
      authorName: s.author,
      authorAvatarDataUrl: s.avatar,
      body: s.body,
      createdAtMs: s.createdAtMs,
      edited: s.edited,
      own: s.author === this.account.name(),
    };
  }
}

/** The account port: the status it answers is the backend's; `signIn` resolves at once. */
export class FakeCommunityAccount implements CommunityAccount {
  constructor(public current: CommunityStatus) {}

  readonly status = vi.fn(() => Promise.resolve(this.current));

  readonly signIn = vi.fn(() => {
    this.current = { state: 'signed_in', profile: { displayName: ME, avatarDataUrl: null } };
    return Promise.resolve(this.current);
  });

  readonly signOut = vi.fn(() => {
    this.current = { state: 'signed_out' };
    return Promise.resolve();
  });

  /** The session the backend holds right now, by display name. */
  name(): string | null {
    return this.current.state === 'signed_in' ? this.current.profile.displayName : null;
  }

  /** The server forgot the session (expired or revoked) without the UI knowing yet. */
  expire(): void {
    this.current = { state: 'signed_out' };
  }
}

export const signedIn = (): CommunityStatus => ({
  state: 'signed_in',
  profile: { displayName: ME, avatarDataUrl: null },
});
export const signedOut = (): CommunityStatus => ({ state: 'signed_out' });
