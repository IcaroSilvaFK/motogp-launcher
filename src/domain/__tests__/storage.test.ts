import { describe, expect, it } from 'vitest';
import { storageBreakdown, storageShare } from '../storage';
import { STORAGE_CATEGORIES } from '../types';
import type { StorageUsage } from '../types';

describe('Given a category size and the total used', () => {
  it('When the total is positive Then the share is the ratio', () => {
    expect(storageShare(25, 100)).toBe(0.25);
    expect(storageShare(100, 100)).toBe(1);
  });

  it('When the total is zero Then the share is zero, never NaN', () => {
    expect(storageShare(0, 0)).toBe(0);
    expect(storageShare(10, 0)).toBe(0);
  });

  it('When the values are out of range Then the share is clamped to 0..1', () => {
    expect(storageShare(150, 100)).toBe(1);
    expect(storageShare(-5, 100)).toBe(0);
    expect(storageShare(Number.NaN, 100)).toBe(0);
  });
});

describe('Given a storage usage', () => {
  const usage = (categories: StorageUsage['categories'], totalBytes: number): StorageUsage => ({
    // An accented Windows profile folder on purpose: the rule must not care about the path.
    root: 'C:/Users/Usuário Padrão/AppData/Roaming/motogp-launcher',
    totalBytes,
    freeBytes: null,
    categories,
  });

  it('Then the display order is mods, launcher data, other', () => {
    expect(STORAGE_CATEGORIES).toEqual(['mods', 'launcher_data', 'other']);
  });

  it('When every category is reported Then the breakdown keeps the fixed order with each share', () => {
    const rows = storageBreakdown(
      usage(
        [
          { category: 'other', bytes: 5 },
          { category: 'mods', bytes: 300 },
          { category: 'launcher_data', bytes: 20 },
        ],
        325,
      ),
    );
    expect(rows).toEqual([
      { category: 'mods', bytes: 300, share: 300 / 325 },
      { category: 'launcher_data', bytes: 20, share: 20 / 325 },
      { category: 'other', bytes: 5, share: 5 / 325 },
    ]);
  });

  it('When a category is missing Then it is listed with 0 bytes', () => {
    const rows = storageBreakdown(usage([{ category: 'mods', bytes: 10 }], 10));
    expect(rows.map((row) => [row.category, row.bytes, row.share])).toEqual([
      ['mods', 10, 1],
      ['launcher_data', 0, 0],
      ['other', 0, 0],
    ]);
  });

  it('When the folder is empty Then every share is zero', () => {
    const rows = storageBreakdown(usage([], 0));
    expect(rows.every((row) => row.bytes === 0 && row.share === 0)).toBe(true);
  });
});
