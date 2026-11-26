import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ModCommentsError } from '../../domain';
import type { CommentsPage, ModComment } from '../../domain';
import type { ModComments } from '../ports/ModComments';
import { useModComments } from '../useModComments';

function comment(id: string, overrides: Partial<ModComment> = {}): ModComment {
  return {
    id,
    authorName: 'Ada',
    authorAvatarDataUrl: null,
    body: `body ${id}`,
    createdAtMs: Number(id.replace(/\D/g, '')) || 0,
    edited: false,
    own: false,
    ...overrides,
  };
}

function page(ids: string[], nextCursor: string | null = null): CommentsPage {
  return { comments: ids.map((id) => comment(id)), nextCursor };
}

function port(overrides: Partial<ModComments> = {}) {
  return {
    list: vi.fn(() => Promise.resolve(page([]))),
    post: vi.fn((_mod: string, body: string) => Promise.resolve(comment('new', { body, own: true }))),
    edit: vi.fn((_mod: string, id: string, body: string) =>
      Promise.resolve(comment(id, { body, edited: true, own: true })),
    ),
    remove: vi.fn(() => Promise.resolve()),
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function ready(p: ModComments, modId = 'red-livery') {
  const hook = renderHook(({ port: current, id }) => useModComments(current, id), {
    initialProps: { port: p as ModComments | undefined, id: modId as string | undefined },
  });
  await waitFor(() => expect(hook.result.current.status).not.toBe('loading'));
  return hook;
}

describe('useModComments', () => {
  it('is unavailable and requests nothing without a port or a mod id', () => {
    const none = renderHook(() => useModComments(undefined, 'm'));
    expect(none.result.current.available).toBe(false);
    const p = port();
    const noMod = renderHook(() => useModComments(p, undefined));
    expect(noMod.result.current.available).toBe(false);
    expect(p.list).not.toHaveBeenCalled();
  });

  it('loads the first page: loading, then ready in order', async () => {
    const p = port({ list: vi.fn(() => Promise.resolve(page(['c1', 'c2', 'c3']))) });
    const { result } = renderHook(() => useModComments(p, 'red-livery'));
    expect(result.current.status).toBe('loading');
    await waitFor(() => expect(result.current.status).toBe('ready'));
    expect(result.current.comments.map((c) => c.id)).toEqual(['c1', 'c2', 'c3']);
    expect(result.current.hasMore).toBe(false);
    expect(p.list).toHaveBeenCalledWith('red-livery', null);
  });

  it('shows offline when the first page fails, and reload retries', async () => {
    const list = vi
      .fn()
      .mockRejectedValueOnce(new ModCommentsError('offline'))
      .mockResolvedValueOnce(page(['c1']));
    const { result } = await ready(port({ list }));
    expect(result.current.status).toBe('offline');
    expect(result.current.comments).toEqual([]);
    act(() => result.current.reload());
    await waitFor(() => expect(result.current.status).toBe('ready'));
    expect(result.current.comments).toHaveLength(1);
  });

  it('survives a port that throws synchronously', async () => {
    const { result } = await ready(
      port({
        list: vi.fn(() => {
          throw new Error('boom');
        }),
      }),
    );
    expect(result.current.status).toBe('offline');
  });

  it('loads more in posting order and drops the cursor at the end', async () => {
    const ids = Array.from({ length: 20 }, (_, i) => `c${i + 1}`);
    const list = vi
      .fn()
      .mockResolvedValueOnce(page(ids, 'cur2'))
      .mockResolvedValueOnce(page(['c21', 'c22', 'c23', 'c24', 'c25'], null));
    const { result } = await ready(port({ list }));
    expect(result.current.hasMore).toBe(true);
    expect(result.current.comments).toHaveLength(20);
    act(() => result.current.loadMore());
    expect(result.current.loadingMore).toBe(true);
    await waitFor(() => expect(result.current.loadingMore).toBe(false));
    expect(list).toHaveBeenLastCalledWith('red-livery', 'cur2');
    expect(result.current.comments.map((c) => c.id)).toEqual([...ids, 'c21', 'c22', 'c23', 'c24', 'c25']);
    expect(result.current.hasMore).toBe(false);
  });

  it('ignores loadMore without a next page and while one is loading', async () => {
    const more = deferred<CommentsPage>();
    const list = vi
      .fn()
      .mockResolvedValueOnce(page(['c1'], 'cur2'))
      .mockReturnValueOnce(more.promise);
    const { result } = await ready(port({ list }));
    act(() => result.current.loadMore());
    act(() => result.current.loadMore());
    expect(list).toHaveBeenCalledTimes(2);
    more.resolve(page(['c2']));
    await waitFor(() => expect(result.current.loadingMore).toBe(false));
    act(() => result.current.loadMore());
    expect(list).toHaveBeenCalledTimes(2);
  });

  it('keeps the list and reports the error when loadMore fails', async () => {
    const list = vi
      .fn()
      .mockResolvedValueOnce(page(['c1'], 'cur2'))
      .mockRejectedValueOnce(new ModCommentsError('offline'));
    const { result } = await ready(port({ list }));
    act(() => result.current.loadMore());
    await waitFor(() => expect(result.current.error?.code).toBe('offline'));
    expect(result.current.status).toBe('ready');
    expect(result.current.comments).toHaveLength(1);
    expect(result.current.hasMore).toBe(true);
    expect(result.current.loadingMore).toBe(false);
  });

  it('does not duplicate a comment posted before the next page arrives', async () => {
    const list = vi
      .fn()
      .mockResolvedValueOnce(page(['c1'], 'cur2'))
      .mockResolvedValueOnce(page(['c2', 'new']));
    const p = port({ list });
    const { result } = await ready(p);
    await act(() => result.current.post('Hello'));
    expect(result.current.comments.map((c) => c.id)).toEqual(['c1', 'new']);
    act(() => result.current.loadMore());
    await waitFor(() => expect(result.current.loadingMore).toBe(false));
    expect(result.current.comments.map((c) => c.id)).toEqual(['c1', 'c2', 'new']);
  });

  it('appends a posted comment at the end', async () => {
    const p = port({ list: vi.fn(() => Promise.resolve(page(['c1', 'c2']))) });
    const { result } = await ready(p);
    let stored = false;
    await act(async () => {
      stored = await result.current.post('Great livery');
    });
    expect(stored).toBe(true);
    expect(p.post).toHaveBeenCalledWith('red-livery', 'Great livery');
    expect(result.current.comments.map((c) => c.id)).toEqual(['c1', 'c2', 'new']);
    expect(result.current.comments[2]).toMatchObject({ body: 'Great livery', own: true });
    expect(result.current.busy).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('rejects an empty or over-long draft locally without calling the port', async () => {
    const p = port();
    const { result } = await ready(p);
    for (const body of ['   ', 'a'.repeat(2001)]) {
      let stored = true;
      await act(async () => {
        stored = await result.current.post(body);
      });
      expect(stored).toBe(false);
      expect(result.current.error?.code).toBe('invalid');
    }
    await act(async () => {
      await result.current.edit('c1', '');
    });
    expect(p.post).not.toHaveBeenCalled();
    expect(p.edit).not.toHaveBeenCalled();
  });

  it('reports rate limiting with the wait and keeps the list', async () => {
    const p = port({
      list: vi.fn(() => Promise.resolve(page(['c1']))),
      post: vi.fn(() => Promise.reject(new ModCommentsError('rate_limited', 30))),
    });
    const { result } = await ready(p);
    let stored = true;
    await act(async () => {
      stored = await result.current.post('x');
    });
    expect(stored).toBe(false);
    expect(result.current.error).toMatchObject({ code: 'rate_limited', retryAfterSecs: 30 });
    expect(result.current.comments).toHaveLength(1);
    expect(result.current.busy).toBe(false);
    act(() => result.current.clearError());
    expect(result.current.error).toBeNull();
  });

  it('reports signed_out and typed failures from a write, mapping odd rejections to failed', async () => {
    const post = vi
      .fn()
      .mockRejectedValueOnce(new ModCommentsError('signed_out'))
      .mockRejectedValueOnce({ code: 'weird' });
    const { result } = await ready(port({ post }));
    await act(async () => {
      await result.current.post('x');
    });
    expect(result.current.error?.code).toBe('signed_out');
    await act(async () => {
      await result.current.post('x');
    });
    expect(result.current.error?.code).toBe('failed');
  });

  it('clears the previous error when a new action starts', async () => {
    const post = vi
      .fn()
      .mockRejectedValueOnce(new ModCommentsError('offline'))
      .mockResolvedValueOnce(comment('new', { own: true }));
    const { result } = await ready(port({ post }));
    await act(async () => {
      await result.current.post('x');
    });
    expect(result.current.error?.code).toBe('offline');
    await act(async () => {
      await result.current.post('x');
    });
    expect(result.current.error).toBeNull();
  });

  it('runs one write at a time', async () => {
    const slow = deferred<ModComment>();
    const p = port({ post: vi.fn(() => slow.promise) });
    const { result } = await ready(p);
    let first!: Promise<boolean>;
    act(() => {
      first = result.current.post('one');
    });
    expect(result.current.busy).toBe(true);
    let second = true;
    await act(async () => {
      second = await result.current.post('two');
    });
    expect(second).toBe(false);
    expect(p.post).toHaveBeenCalledTimes(1);
    await act(async () => {
      slow.resolve(comment('new', { own: true }));
      await first;
    });
    expect(result.current.busy).toBe(false);
  });

  it('edits a comment in place and marks it edited', async () => {
    const own = comment('c2', { own: true });
    const p = port({ list: vi.fn(() => Promise.resolve({ comments: [comment('c1'), own], nextCursor: null })) });
    const { result } = await ready(p);
    await act(async () => {
      expect(await result.current.edit('c2', 'Nice livery')).toBe(true);
    });
    expect(p.edit).toHaveBeenCalledWith('red-livery', 'c2', 'Nice livery');
    expect(result.current.comments[1]).toMatchObject({ id: 'c2', body: 'Nice livery', edited: true });
    expect(result.current.comments[0]?.body).toBe('body c1');
  });

  it('removes a comment', async () => {
    const p = port({ list: vi.fn(() => Promise.resolve(page(['c1', 'c2']))) });
    const { result } = await ready(p);
    await act(async () => {
      expect(await result.current.remove('c1')).toBe(true);
    });
    expect(p.remove).toHaveBeenCalledWith('red-livery', 'c1');
    expect(result.current.comments.map((c) => c.id)).toEqual(['c2']);
  });

  it('drops a comment the server no longer has and reports not_found', async () => {
    const p = port({
      list: vi.fn(() => Promise.resolve(page(['c1', 'c2']))),
      remove: vi.fn(() => Promise.reject(new ModCommentsError('not_found'))),
      edit: vi.fn(() => Promise.reject(new ModCommentsError('not_found'))),
    });
    const { result } = await ready(p);
    await act(async () => {
      expect(await result.current.remove('c1')).toBe(false);
    });
    expect(result.current.error?.code).toBe('not_found');
    expect(result.current.comments.map((c) => c.id)).toEqual(['c2']);
    await act(async () => {
      await result.current.edit('c2', 'x');
    });
    expect(result.current.comments).toEqual([]);
  });

  it('does not write before the first page is ready', async () => {
    const first = deferred<CommentsPage>();
    const p = port({ list: vi.fn(() => first.promise) });
    const { result } = renderHook(() => useModComments(p, 'm'));
    let stored = true;
    await act(async () => {
      stored = await result.current.post('x');
    });
    expect(stored).toBe(false);
    expect(p.post).not.toHaveBeenCalled();
  });

  it('starts over when the mod changes and ignores late answers of the previous mod', async () => {
    const slow = deferred<CommentsPage>();
    const list = vi.fn((mod: string) => (mod === 'old' ? slow.promise : Promise.resolve(page(['n1']))));
    const p = port({ list });
    const hook = renderHook(({ id }) => useModComments(p, id), { initialProps: { id: 'old' } });
    hook.rerender({ id: 'new' });
    await waitFor(() => expect(hook.result.current.status).toBe('ready'));
    await act(async () => {
      slow.resolve(page(['stale']));
      await slow.promise;
    });
    expect(hook.result.current.comments.map((c) => c.id)).toEqual(['n1']);
  });

  it('ignores answers that arrive after unmount', async () => {
    const slow = deferred<CommentsPage>();
    const p = port({ list: vi.fn(() => slow.promise) });
    const { unmount, result } = renderHook(() => useModComments(p, 'm'));
    unmount();
    await act(async () => {
      slow.resolve(page(['c1']));
      await slow.promise;
    });
    expect(result.current.status).toBe('loading');
  });
});
