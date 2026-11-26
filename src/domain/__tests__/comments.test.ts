import { describe, expect, it } from 'vitest';
import {
  commentDraftProblem,
  MAX_COMMENT_LENGTH,
  ModCommentsError,
  parseCommentsPage,
  parseModComment,
  toModCommentsError,
} from '../comments';

const wire = {
  id: 'c1',
  authorName: 'Ada',
  authorAvatarDataUrl: 'data:image/png;base64,AAAA',
  body: 'Great livery',
  createdAtMs: 1_795_688_130_123,
  edited: false,
  own: true,
};

describe('parseModComment', () => {
  it('accepts a well-formed comment', () => {
    expect(parseModComment(wire)).toEqual(wire);
  });

  it('keeps HTML bodies verbatim (rendering is the UI job)', () => {
    expect(parseModComment({ ...wire, body: '<img src=x onerror=alert(1)>' })?.body).toBe(
      '<img src=x onerror=alert(1)>',
    );
  });

  it('drops an avatar that is not a raster data URL', () => {
    for (const avatar of ['https://evil.example/a.png', 'data:image/svg+xml;base64,AAAA', 42, undefined]) {
      expect(parseModComment({ ...wire, authorAvatarDataUrl: avatar })?.authorAvatarDataUrl).toBeNull();
    }
  });

  it('rejects malformed comments', () => {
    for (const bad of [
      null,
      'x',
      {},
      { ...wire, id: '' },
      { ...wire, authorName: 3 },
      { ...wire, body: null },
      { ...wire, createdAtMs: 'now' },
      { ...wire, createdAtMs: Number.NaN },
      { ...wire, edited: 'no' },
      { ...wire, own: undefined },
    ]) {
      expect(parseModComment(bad)).toBeNull();
    }
  });
});

describe('parseCommentsPage', () => {
  it('parses a page with and without a cursor', () => {
    expect(parseCommentsPage({ comments: [wire], nextCursor: 'n' })).toEqual({ comments: [wire], nextCursor: 'n' });
    expect(parseCommentsPage({ comments: [], nextCursor: null })).toEqual({ comments: [], nextCursor: null });
    expect(parseCommentsPage({ comments: [] })?.nextCursor).toBeNull();
  });

  it('rejects anything that is not a page', () => {
    for (const bad of [
      null,
      1,
      {},
      { comments: 'x' },
      { comments: [], nextCursor: 4 },
      { comments: [wire, { id: 1 }] },
    ]) {
      expect(parseCommentsPage(bad)).toBeNull();
    }
  });
});

describe('toModCommentsError', () => {
  it('maps each backend code', () => {
    const code = (c: string) => toModCommentsError({ code: c, message: '' }).code;
    expect(code('community_offline')).toBe('offline');
    expect(code('community_signed_out')).toBe('signed_out');
    expect(code('comment_rate_limited')).toBe('rate_limited');
    expect(code('comment_invalid')).toBe('invalid');
    expect(code('comment_not_found')).toBe('not_found');
  });

  it('reads the wait from the rate-limit message', () => {
    const error = toModCommentsError({
      code: 'comment_rate_limited',
      message: 'You are commenting too fast. Try again in 42 seconds.',
    });
    expect(error.retryAfterSecs).toBe(42);
    expect(toModCommentsError({ code: 'comment_rate_limited', message: 'Try again in 1 second.' }).retryAfterSecs).toBe(
      1,
    );
  });

  it('falls back to 1 second when the message has no number', () => {
    expect(toModCommentsError({ code: 'comment_rate_limited' }).retryAfterSecs).toBe(1);
  });

  it('has no wait for other codes', () => {
    expect(toModCommentsError({ code: 'comment_invalid', message: 'in 5 seconds.' }).retryAfterSecs).toBeNull();
  });

  it('maps unknown or odd errors to failed and keeps typed errors', () => {
    expect(toModCommentsError({ code: 'weird' }).code).toBe('failed');
    expect(toModCommentsError(new Error('x')).code).toBe('failed');
    expect(toModCommentsError('boom').code).toBe('failed');
    expect(toModCommentsError(null).code).toBe('failed');
    const typed = new ModCommentsError('offline');
    expect(toModCommentsError(typed)).toBe(typed);
  });
});

describe('commentDraftProblem', () => {
  it('accepts a normal comment', () => {
    expect(commentDraftProblem('Great livery')).toBeNull();
  });

  it('rejects empty and blank drafts', () => {
    expect(commentDraftProblem('')).toBe('empty');
    expect(commentDraftProblem(' \r\n\t ')).toBe('empty');
  });

  it('counts characters after trimming, not UTF-16 units', () => {
    expect(commentDraftProblem(`  ${'a'.repeat(MAX_COMMENT_LENGTH)}  `)).toBeNull();
    expect(commentDraftProblem('a'.repeat(MAX_COMMENT_LENGTH + 1))).toBe('too_long');
    // Emoji outside the BMP are one character each (the server counts chars).
    expect(commentDraftProblem('😀'.repeat(MAX_COMMENT_LENGTH))).toBeNull();
    expect(commentDraftProblem('😀'.repeat(MAX_COMMENT_LENGTH + 1))).toBe('too_long');
  });
});
