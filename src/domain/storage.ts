import { STORAGE_CATEGORIES } from './types';
import type { StorageCategory, StorageUsage } from './types';

/** Fraction (0..1) of `total` used by `bytes`; 0 when the total is 0 or a value is not a number. */
export function storageShare(bytes: number, total: number): number {
  if (!(total > 0) || !(bytes > 0)) return 0;
  return Math.min(1, bytes / total);
}

export interface StorageShare {
  category: StorageCategory;
  bytes: number;
  share: number;
}

/** One row per category in the fixed display order; a category the usage does not list counts as 0 bytes. */
export function storageBreakdown(usage: StorageUsage): StorageShare[] {
  return STORAGE_CATEGORIES.map((category) => {
    const bytes = usage.categories
      .filter((entry) => entry.category === category)
      .reduce((sum, entry) => sum + entry.bytes, 0);
    return { category, bytes, share: storageShare(bytes, usage.totalBytes) };
  });
}
