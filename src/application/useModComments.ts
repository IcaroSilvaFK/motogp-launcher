import { useCallback, useEffect, useRef, useState } from 'react';
import { commentDraftProblem, MAX_REPORT_DETAILS_LENGTH, ModCommentsError, toModCommentsError } from '../domain';
import type { ModComment, ReportReason } from '../domain';
import type { ModComments } from './ports/ModComments';

/** `loading`: first page in flight; `ready`: the list is usable (maybe empty); `offline`: the first page failed. */
export type ModCommentsStatus = 'loading' | 'ready' | 'offline';

/**
 * What `useModComments` returns.
 *
 * Typical UI: show a skeleton while `status === 'loading'`, a quiet message while `'offline'`, the
 * `comments` (oldest first, newest last) while `'ready'`, a "Load more" button while `hasMore`
 * (disabled while `loadingMore`), and a composer that calls `post` (disabled while `busy`).
 * `error` is the last failed action and stays until the next action starts or `clearError()`:
 * `signed_out` (offer sign-in; refresh the account status), `rate_limited` (show `retryAfterSecs`),
 * `invalid`, `not_found` (the comment is already dropped from the list), `banned` (the account is
 * suspended: show the suspension message), `offline` or `failed`.
 */
export interface ModCommentsHandle {
  /** False without a port or a mod id (tests, builds without the community): render nothing. */
  available: boolean;
  status: ModCommentsStatus;
  /** Loaded comments, oldest first. A posted comment is appended at the end. */
  comments: ModComment[];
  /** A further page exists: call `loadMore`. */
  hasMore: boolean;
  loadingMore: boolean;
  /** A post, edit or delete is in flight (one write at a time). */
  busy: boolean;
  /** The last failed action (`loadMore`, `post`, `edit` or `remove`); `null` when none. */
  error: ModCommentsError | null;
  /** Reads the first page again (the "retry" of the offline state). */
  reload(): void;
  /** Reads the next page and appends it. No-op without `hasMore` or while loading. */
  loadMore(): void;
  /** Resolves `true` when stored. Validates locally first (empty / over 2000 characters: `invalid`, no call). */
  post(body: string): Promise<boolean>;
  /** Resolves `true` when saved; the comment is replaced in place (`edited: true`). */
  edit(commentId: string, body: string): Promise<boolean>;
  /**
   * Reports someone else's comment (the list is unchanged). Resolves `true` when the server took it;
   * show a "Reported" confirmation. Details over 500 characters resolve `false` with `invalid`, no call.
   */
  report(commentId: string, reason: ReportReason, details?: string): Promise<boolean>;
  /** Resolves `true` when deleted. Confirm with the user before calling. */
  remove(commentId: string): Promise<boolean>;
  clearError(): void;
}

interface Data {
  status: ModCommentsStatus;
  comments: ModComment[];
  nextCursor: string | null;
}

const LOADING: Data = { status: 'loading', comments: [], nextCursor: null };

/**
 * Loads the comments of one mod in an effect (nothing awaits it) and drives pagination and writes.
 * A new `modId` starts over; answers that arrive for a previous mod are ignored. Without a port or a
 * mod id nothing is requested and the handle stays `available: false`.
 */
export function useModComments(port: ModComments | undefined, modId: string | undefined): ModCommentsHandle {
  const [data, setData] = useState<Data>(LOADING);
  const [loadingMore, setLoadingMore] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ModCommentsError | null>(null);
  const [reloads, setReloads] = useState(0);
  // Bumped whenever the mod changes or the first page is re-read: older answers are dropped.
  const generation = useRef(0);
  const loadingMoreRef = useRef(false);
  const busyRef = useRef(false);

  // A new mod, port or reload starts over. Resetting during render (not in an effect) keeps the
  // stale list from ever showing for the new scope.
  const [scope, setScope] = useState({ port, modId, reloads });
  if (scope.port !== port || scope.modId !== modId || scope.reloads !== reloads) {
    setScope({ port, modId, reloads });
    setData(LOADING);
    setLoadingMore(false);
    setBusy(false);
    setError(null);
  }

  useEffect(() => {
    generation.current += 1;
    const mine = generation.current;
    loadingMoreRef.current = false;
    busyRef.current = false;
    if (!port || !modId) return;
    let attempt: Promise<Awaited<ReturnType<ModComments['list']>>>;
    try {
      attempt = port.list(modId, null);
    } catch (thrown) {
      attempt = Promise.reject(thrown);
    }
    attempt.then(
      (page) => {
        if (generation.current !== mine) return;
        setData({ status: 'ready', comments: page.comments, nextCursor: page.nextCursor });
      },
      () => {
        if (generation.current !== mine) return;
        setData({ status: 'offline', comments: [], nextCursor: null });
      },
    );
    return () => {
      // Also invalidates answers after unmount.
      generation.current += 1;
    };
  }, [port, modId, reloads]);

  const reload = useCallback(() => setReloads((n) => n + 1), []);

  const loadMore = useCallback(() => {
    if (!port || !modId || data.status !== 'ready' || !data.nextCursor || loadingMoreRef.current) return;
    const mine = generation.current;
    loadingMoreRef.current = true;
    setLoadingMore(true);
    setError(null);
    let attempt: ReturnType<ModComments['list']>;
    try {
      attempt = port.list(modId, data.nextCursor);
    } catch (thrown) {
      attempt = Promise.reject(thrown);
    }
    attempt.then(
      (page) => {
        if (generation.current !== mine) return;
        loadingMoreRef.current = false;
        setLoadingMore(false);
        setData((previous) => {
          // A comment posted meanwhile is also in the page: the server's order wins.
          const incoming = new Set(page.comments.map((c) => c.id));
          const kept = previous.comments.filter((c) => !incoming.has(c.id));
          return { status: 'ready', comments: [...kept, ...page.comments], nextCursor: page.nextCursor };
        });
      },
      (thrown: unknown) => {
        if (generation.current !== mine) return;
        loadingMoreRef.current = false;
        setLoadingMore(false);
        setError(toModCommentsError(thrown));
      },
    );
  }, [port, modId, data]);

  /** Runs one write at a time; `apply` updates the list when it succeeded. */
  const write = useCallback(
    (run: () => Promise<ModComment | void>, apply: (result: ModComment | void) => void, dropOnMissing?: string) => {
      if (busyRef.current) return Promise.resolve(false);
      const mine = generation.current;
      busyRef.current = true;
      setBusy(true);
      setError(null);
      let attempt: Promise<ModComment | void>;
      try {
        attempt = run();
      } catch (thrown) {
        attempt = Promise.reject(thrown);
      }
      return attempt.then(
        (result) => {
          if (generation.current !== mine) return true;
          busyRef.current = false;
          setBusy(false);
          apply(result);
          return true;
        },
        (thrown: unknown) => {
          if (generation.current !== mine) return false;
          busyRef.current = false;
          setBusy(false);
          const failure = toModCommentsError(thrown);
          setError(failure);
          if (failure.code === 'not_found' && dropOnMissing !== undefined) {
            setData((previous) => ({
              ...previous,
              comments: previous.comments.filter((c) => c.id !== dropOnMissing),
            }));
          }
          return false;
        },
      );
    },
    [],
  );

  const rejectDraft = useCallback((body: string) => {
    if (commentDraftProblem(body) === null) return false;
    setError(new ModCommentsError('invalid'));
    return true;
  }, []);

  const post = useCallback(
    (body: string) => {
      if (!port || !modId || data.status !== 'ready') return Promise.resolve(false);
      if (rejectDraft(body)) return Promise.resolve(false);
      return write(
        () => port.post(modId, body),
        (created) => {
          if (!created) return;
          setData((previous) => ({
            ...previous,
            comments: [...previous.comments.filter((c) => c.id !== created.id), created],
          }));
        },
      );
    },
    [port, modId, data.status, write, rejectDraft],
  );

  const edit = useCallback(
    (commentId: string, body: string) => {
      if (!port || !modId || data.status !== 'ready') return Promise.resolve(false);
      if (rejectDraft(body)) return Promise.resolve(false);
      return write(
        () => port.edit(modId, commentId, body),
        (updated) => {
          if (!updated) return;
          setData((previous) => ({
            ...previous,
            comments: previous.comments.map((c) => (c.id === updated.id ? updated : c)),
          }));
        },
        commentId,
      );
    },
    [port, modId, data.status, write, rejectDraft],
  );

  const remove = useCallback(
    (commentId: string) => {
      if (!port || !modId || data.status !== 'ready') return Promise.resolve(false);
      return write(
        () => port.remove(modId, commentId),
        () =>
          setData((previous) => ({
            ...previous,
            comments: previous.comments.filter((c) => c.id !== commentId),
          })),
        commentId,
      );
    },
    [port, modId, data.status, write],
  );

  const report = useCallback(
    (commentId: string, reason: ReportReason, details?: string) => {
      if (!port || !modId || data.status !== 'ready') return Promise.resolve(false);
      if ([...(details ?? '').trim()].length > MAX_REPORT_DETAILS_LENGTH) {
        setError(new ModCommentsError('invalid'));
        return Promise.resolve(false);
      }
      return write(
        () => port.report(modId, commentId, reason, details),
        () => undefined,
        commentId,
      );
    },
    [port, modId, data.status, write],
  );

  const clearError = useCallback(() => setError(null), []);

  return {
    available: port !== undefined && modId !== undefined,
    status: data.status,
    comments: data.comments,
    hasMore: data.nextCursor !== null,
    loadingMore,
    busy,
    error,
    reload,
    loadMore,
    post,
    edit,
    remove,
    report,
    clearError,
  };
}
