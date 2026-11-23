import { describe, expect, it } from 'vitest';
import { isSupportedModFile, isVanilla, moveItem, SUPPORTED_MOD_EXTENSIONS, toCommandError, toggleMod } from '../rules';

describe('toCommandError', () => {
  it('keeps a {code, message} object', () => {
    expect(toCommandError({ code: 'missing_app_id', message: 'no app id' })).toEqual({
      code: 'missing_app_id',
      message: 'no app id',
    });
  });

  it('maps an unknown code to unknown but keeps the message', () => {
    expect(toCommandError({ code: 'weird', message: 'boom' })).toEqual({ code: 'unknown', message: 'boom' });
  });

  it('normalizes an Error', () => {
    expect(toCommandError(new Error('bad'))).toEqual({ code: 'unknown', message: 'bad' });
  });

  it('normalizes a string', () => {
    expect(toCommandError('plain')).toEqual({ code: 'unknown', message: 'plain' });
  });

  it('normalizes anything else', () => {
    expect(toCommandError(42)).toEqual({ code: 'unknown', message: '42' });
    expect(toCommandError(null)).toEqual({ code: 'unknown', message: 'null' });
    expect(toCommandError({ code: 'read_failed' })).toEqual({ code: 'read_failed', message: '' });
  });
});

describe('isSupportedModFile', () => {
  it('lists the supported extensions', () => {
    expect(SUPPORTED_MOD_EXTENSIONS).toEqual(['pak', 'utoc', 'ucas', 'zip']);
  });

  it('accepts supported extensions case-insensitively', () => {
    expect(isSupportedModFile('a.pak')).toBe(true);
    expect(isSupportedModFile('a.UTOC')).toBe(true);
    expect(isSupportedModFile('a.Ucas')).toBe(true);
    expect(isSupportedModFile('a.ZIP')).toBe(true);
  });

  it('accepts paths with spaces and accents', () => {
    // The accent in "Usuário Padrão" is on purpose: real Windows users have accented paths.
    expect(isSupportedModFile('C:/Users/Usuário Padrão/mod.PAK')).toBe(true);
    expect(isSupportedModFile('C:\\Users\\Usuário Padrão\\my mod.zip')).toBe(true);
  });

  it('rejects other files', () => {
    expect(isSupportedModFile('readme.txt')).toBe(false);
    expect(isSupportedModFile('noextension')).toBe(false);
    expect(isSupportedModFile('pak')).toBe(false);
    expect(isSupportedModFile('C:/dir.pak/file')).toBe(false);
    expect(isSupportedModFile('a.pak.txt')).toBe(false);
  });
});

describe('moveItem', () => {
  it('moves an item without mutating the input', () => {
    const list = ['a', 'b', 'c'];
    expect(moveItem(list, 1, -1)).toEqual(['b', 'a', 'c']);
    expect(moveItem(list, 1, 1)).toEqual(['a', 'c', 'b']);
    expect(list).toEqual(['a', 'b', 'c']);
  });

  it('returns a copy when the move is out of range', () => {
    const list = ['a', 'b'];
    const up = moveItem(list, 0, -1);
    const down = moveItem(list, 1, 1);
    expect(up).toEqual(['a', 'b']);
    expect(down).toEqual(['a', 'b']);
    expect(up).not.toBe(list);
    expect(moveItem(list, 5, -1)).toEqual(['a', 'b']);
    expect(moveItem(list, -1, 1)).toEqual(['a', 'b']);
  });
});

describe('toggleMod', () => {
  it('appends a missing id keeping the order', () => {
    expect(toggleMod(['b'], 'a')).toEqual(['b', 'a']);
  });

  it('removes a present id keeping the order', () => {
    const ids = ['b', 'a', 'c'];
    expect(toggleMod(ids, 'a')).toEqual(['b', 'c']);
    expect(ids).toEqual(['b', 'a', 'c']);
  });
});

describe('isVanilla', () => {
  it('detects the vanilla profile by id', () => {
    expect(isVanilla({ id: 'vanilla', name: 'Vanilla', mod_ids: [] })).toBe(true);
    expect(isVanilla({ id: 'x', name: 'Vanilla', mod_ids: [] })).toBe(false);
  });
});
