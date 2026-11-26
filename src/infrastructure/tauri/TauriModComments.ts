import { invoke } from '@tauri-apps/api/core';
import type { ModComments } from '../../application';
import { ModCommentsError, parseCommentsPage, parseModComment, toModCommentsError } from '../../domain';

async function call(command: string, args: Record<string, unknown>): Promise<unknown> {
  try {
    return await invoke(command, args);
  } catch (error) {
    throw toModCommentsError(error);
  }
}

/**
 * Mod comments over IPC (`community_comments_list`, `community_comment_post`, `community_comment_edit`,
 * `community_comment_delete`, `community_report_comment`). Rejections are typed `ModCommentsError`s; a malformed answer is `failed`.
 */
export function createTauriModComments(): ModComments {
  return {
    async list(modId, cursor) {
      const args: Record<string, unknown> = { modId };
      if (cursor) args.cursor = cursor;
      const page = parseCommentsPage(await call('community_comments_list', args));
      if (!page) throw new ModCommentsError('failed');
      return page;
    },
    async post(modId, body) {
      const comment = parseModComment(await call('community_comment_post', { modId, body }));
      if (!comment) throw new ModCommentsError('failed');
      return comment;
    },
    async edit(modId, commentId, body) {
      const comment = parseModComment(await call('community_comment_edit', { modId, commentId, body }));
      if (!comment) throw new ModCommentsError('failed');
      return comment;
    },
    async remove(modId, commentId) {
      await call('community_comment_delete', { modId, commentId });
    },
    async report(modId, commentId, reason, details) {
      const args: Record<string, unknown> = { modId, commentId, reason };
      const trimmed = details?.trim();
      if (trimmed) args.details = trimmed;
      await call('community_report_comment', args);
    },
  };
}
