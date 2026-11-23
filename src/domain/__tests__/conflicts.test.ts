import { describe, expect, it } from 'vitest';
import { conflictsOfMod, modConflictStatus } from '../rules';
import type { FileConflict } from '../types';

const conflicts: FileConflict[] = [
  { path: 'a.pak', mod_ids: ['a', 'b'], winner: 'b' },
  { path: 'c.pak', mod_ids: ['b', 'c'], winner: 'c' },
  { path: 'd.pak', mod_ids: ['d', 'e'], winner: 'e' },
];

describe('modConflictStatus', () => {
  it('is overridden when another mod wins a file', () => {
    expect(modConflictStatus(conflicts, 'a')).toBe('overridden');
  });

  it('is overridden when it wins one file but loses another', () => {
    expect(modConflictStatus(conflicts, 'b')).toBe('overridden');
  });

  it('is wins when it only wins', () => {
    expect(modConflictStatus(conflicts, 'e')).toBe('wins');
  });

  it('is none for an uninvolved mod or without conflicts', () => {
    expect(modConflictStatus(conflicts, 'z')).toBe('none');
    expect(modConflictStatus([], 'a')).toBe('none');
  });
});

describe('conflictsOfMod', () => {
  it('lists only the conflicts involving the mod', () => {
    expect(conflictsOfMod(conflicts, 'b').map((c) => c.path)).toEqual(['a.pak', 'c.pak']);
    expect(conflictsOfMod(conflicts, 'z')).toEqual([]);
  });
});
