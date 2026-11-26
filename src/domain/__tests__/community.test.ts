import { describe, expect, it } from 'vitest';
import { CommunitySignInError, parseCommunityStatus, toCommunitySignInError } from '../community';

const avatar = 'data:image/png;base64,AAAA';

describe('parseCommunityStatus', () => {
  it('reads the states without a profile', () => {
    expect(parseCommunityStatus({ state: 'signed_out' })).toEqual({ state: 'signed_out' });
    expect(parseCommunityStatus({ state: 'offline' })).toEqual({ state: 'offline' });
  });

  it('reads a signed-in profile with its avatar', () => {
    expect(
      parseCommunityStatus({ state: 'signed_in', profile: { displayName: 'rider', avatarDataUrl: avatar } }),
    ).toEqual({ state: 'signed_in', profile: { displayName: 'rider', avatarDataUrl: avatar } });
  });

  it('keeps the profile but drops a missing or unsafe avatar', () => {
    for (const avatarDataUrl of [null, undefined, 'https://cdn.example/a.png', 'data:image/svg+xml;base64,AAAA', 3]) {
      expect(parseCommunityStatus({ state: 'signed_in', profile: { displayName: 'rider', avatarDataUrl } })).toEqual({
        state: 'signed_in',
        profile: { displayName: 'rider', avatarDataUrl: null },
      });
    }
  });

  it('refuses anything that is not a known state', () => {
    for (const value of [
      null,
      undefined,
      'signed_in',
      42,
      {},
      { state: 'bogus' },
      { state: 'signed_in' },
      { state: 'signed_in', profile: null },
      { state: 'signed_in', profile: { displayName: '  ' } },
      { state: 'signed_in', profile: { displayName: 7 } },
    ]) {
      expect(parseCommunityStatus(value)).toBeNull();
    }
  });
});

describe('toCommunitySignInError', () => {
  it('maps every backend code', () => {
    const codes = {
      community_offline: 'offline',
      community_sign_in_failed: 'failed',
      community_sign_in_timeout: 'timeout',
      community_sign_in_denied: 'denied',
    };
    for (const [backend, code] of Object.entries(codes)) {
      expect(toCommunitySignInError({ code: backend, message: 'x' }).code).toBe(code);
    }
  });

  it('treats unknown or malformed rejections as failed', () => {
    for (const error of [{ code: 'weird' }, { code: 7 }, 'boom', null, undefined, new Error('x')]) {
      expect(toCommunitySignInError(error).code).toBe('failed');
    }
  });

  it('keeps an error that is already typed', () => {
    const typed = new CommunitySignInError('timeout');
    expect(toCommunitySignInError(typed)).toBe(typed);
    expect(typed).toBeInstanceOf(Error);
  });
});
