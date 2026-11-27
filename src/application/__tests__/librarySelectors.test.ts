import { describe, expect, it } from 'vitest';
import type { FileConflict, ModEntry } from '../../domain';
import { selectConflictingMods, sortMods } from '../librarySelectors';
import { loadLibrarySort, saveLibrarySort } from '../librarySort';

const mod = (name: string, size_bytes: number): ModEntry => ({ id: name.toLowerCase(), name, files: [], size_bytes });
const zeta = mod('Zeta', 10);
const alpha = mod('Alpha', 30);
const mid = mod('Mid', 20);
const names = (mods: ModEntry[]) => mods.map((m) => m.name);

describe('sortMods', () => {
  it('sorts by name A-Z, ignoring case', () => {
    expect(names(sortMods([zeta, mod('beta', 1), alpha, mid], 'name'))).toEqual(['Alpha', 'beta', 'Mid', 'Zeta']);
  });

  it('sorts by size, largest first, with the name as tie-break', () => {
    expect(names(sortMods([zeta, mid, alpha, mod('Other', 20)], 'size'))).toEqual(['Alpha', 'Mid', 'Other', 'Zeta']);
  });

  it('does not mutate its input', () => {
    const input = [zeta, alpha];
    sortMods(input, 'name');
    expect(names(input)).toEqual(['Zeta', 'Alpha']);
  });
});

describe('selectConflictingMods', () => {
  const conflicts: FileConflict[] = [{ path: 'x_P.pak', mod_ids: ['alpha', 'mid'], winner: 'mid' }];

  it('keeps only the mods involved in a conflict, winners and overridden alike', () => {
    expect(names(selectConflictingMods([zeta, alpha, mid], conflicts))).toEqual(['Alpha', 'Mid']);
  });

  it('is empty without conflicts', () => {
    expect(selectConflictingMods([zeta, alpha], [])).toEqual([]);
  });
});

describe('library sort preference', () => {
  it('defaults to name and returns a stored value', () => {
    expect(loadLibrarySort({ load: () => null, save: () => undefined })).toBe('name');
    expect(loadLibrarySort({ load: () => 'size', save: () => undefined })).toBe('size');
  });

  it('survives a throwing store', () => {
    const broken = {
      load: () => {
        throw new Error('blocked');
      },
      save: () => {
        throw new Error('blocked');
      },
    };
    expect(loadLibrarySort(broken)).toBe('name');
    expect(() => saveLibrarySort(broken, 'size')).not.toThrow();
  });
});
