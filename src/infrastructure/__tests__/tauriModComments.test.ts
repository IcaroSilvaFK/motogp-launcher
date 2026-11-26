import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import type { ModCommentsError } from '../../domain';
import { createTauriModComments } from '../tauri/TauriModComments';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

const comment = {
  id: 'c1',
  authorName: 'Ada',
  authorAvatarDataUrl: null,
  body: 'Great livery',
  createdAtMs: 1000,
  edited: false,
  own: true,
};

beforeEach(() => invokeMock.mockReset());
afterEach(() => vi.restoreAllMocks());

async function rejection(promise: Promise<unknown>): Promise<ModCommentsError> {
  try {
    await promise;
  } catch (error) {
    return error as ModCommentsError;
  }
  throw new Error('expected a rejection');
}

describe('createTauriModComments', () => {
  it('lists the first page without a cursor', async () => {
    invokeMock.mockResolvedValueOnce({ comments: [comment], nextCursor: 'n' });
    const page = await createTauriModComments().list('red-livery');
    expect(page).toEqual({ comments: [comment], nextCursor: 'n' });
    expect(invokeMock).toHaveBeenCalledWith('community_comments_list', { modId: 'red-livery' });
  });

  it('passes the cursor on', async () => {
    invokeMock.mockResolvedValueOnce({ comments: [], nextCursor: null });
    await createTauriModComments().list('red-livery', 'cur');
    expect(invokeMock).toHaveBeenCalledWith('community_comments_list', { modId: 'red-livery', cursor: 'cur' });
  });

  it('posts, edits and deletes with camelCase arguments and never sends a token', async () => {
    const port = createTauriModComments();
    invokeMock.mockResolvedValueOnce(comment);
    expect(await port.post('m', 'hi')).toEqual(comment);
    expect(invokeMock).toHaveBeenLastCalledWith('community_comment_post', { modId: 'm', body: 'hi' });
    invokeMock.mockResolvedValueOnce({ ...comment, edited: true });
    expect((await port.edit('m', 'c1', 'yo')).edited).toBe(true);
    expect(invokeMock).toHaveBeenLastCalledWith('community_comment_edit', { modId: 'm', commentId: 'c1', body: 'yo' });
    invokeMock.mockResolvedValueOnce(null);
    await port.remove('m', 'c1');
    expect(invokeMock).toHaveBeenLastCalledWith('community_comment_delete', { modId: 'm', commentId: 'c1' });
    for (const call of invokeMock.mock.calls) expect(JSON.stringify(call)).not.toMatch(/token/i);
  });

  it('reports a comment over IPC with trimmed details and resolves with nothing', async () => {
    invokeMock.mockResolvedValueOnce(null);
    await expect(createTauriModComments().report('m', 'c1', 'off_topic', '  not a livery ')).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith('community_report_comment', {
      modId: 'm',
      commentId: 'c1',
      reason: 'off_topic',
      details: 'not a livery',
    });
    invokeMock.mockResolvedValueOnce(null);
    await createTauriModComments().report('m', 'c1', 'spam', '   ');
    expect(invokeMock).toHaveBeenLastCalledWith('community_report_comment', {
      modId: 'm',
      commentId: 'c1',
      reason: 'spam',
    });
  });

  it('maps a suspended account to banned', async () => {
    invokeMock.mockRejectedValueOnce({ code: 'community_banned', message: 'x' });
    expect((await rejection(createTauriModComments().report('m', 'c', 'abuse'))).code).toBe('banned');
    invokeMock.mockRejectedValueOnce({ code: 'community_banned', message: 'x' });
    expect((await rejection(createTauriModComments().post('m', 'x'))).code).toBe('banned');
  });

  it('turns backend errors into typed errors', async () => {
    invokeMock.mockRejectedValueOnce({ code: 'community_offline', message: 'x' });
    expect((await rejection(createTauriModComments().list('m'))).code).toBe('offline');
    invokeMock.mockRejectedValueOnce({ code: 'community_signed_out', message: 'x' });
    expect((await rejection(createTauriModComments().post('m', 'x'))).code).toBe('signed_out');
    invokeMock.mockRejectedValueOnce({ code: 'comment_rate_limited', message: 'Try again in 9 seconds.' });
    const limited = await rejection(createTauriModComments().post('m', 'x'));
    expect(limited.code).toBe('rate_limited');
    expect(limited.retryAfterSecs).toBe(9);
    invokeMock.mockRejectedValueOnce({ code: 'comment_not_found', message: 'x' });
    expect((await rejection(createTauriModComments().remove('m', 'c'))).code).toBe('not_found');
    invokeMock.mockRejectedValueOnce(new Error('command not found'));
    expect((await rejection(createTauriModComments().list('m'))).code).toBe('failed');
  });

  it('treats a malformed answer as failed', async () => {
    const port = createTauriModComments();
    invokeMock.mockResolvedValueOnce({ comments: 'nope' });
    expect((await rejection(port.list('m'))).code).toBe('failed');
    invokeMock.mockResolvedValueOnce({ id: 1 });
    expect((await rejection(port.post('m', 'x'))).code).toBe('failed');
    invokeMock.mockResolvedValueOnce(null);
    expect((await rejection(port.edit('m', 'c', 'x'))).code).toBe('failed');
  });
});
