import type { CatalogEntry, ModEntry } from '../domain';

/** Pure selectors of the Discover tab (#73): category facets, search, sort and library matching. */

export interface CategoryCount {
  category: string;
  count: number;
}

export type CatalogSort = 'updated' | 'name' | 'stars';

export interface CatalogFilter {
  /** `null` keeps every category. */
  category: string | null;
  /** Matched against title, author and category, case-insensitive; blank keeps everything. */
  query: string;
}

const byName = (a: CatalogEntry, b: CatalogEntry): number =>
  a.title.localeCompare(b.title, undefined, { sensitivity: 'base' });

/** Entries per category, sorted by category name. */
export function categoryCounts(entries: readonly CatalogEntry[]): CategoryCount[] {
  const counts = new Map<string, number>();
  for (const entry of entries) counts.set(entry.category, (counts.get(entry.category) ?? 0) + 1);
  return [...counts]
    .map(([category, count]) => ({ category, count }))
    .sort((a, b) => a.category.localeCompare(b.category, undefined, { sensitivity: 'base' }));
}

export function selectEntries(entries: readonly CatalogEntry[], filter: CatalogFilter): CatalogEntry[] {
  const query = filter.query.trim().toLocaleLowerCase();
  return entries.filter(
    (entry) =>
      (filter.category === null || entry.category === filter.category) &&
      (query === '' ||
        [entry.title, entry.author, entry.category].some((field) => field.toLocaleLowerCase().includes(query))),
  );
}

// `updatedAt` is `YYYY-MM-DD`, so the string order is the date order.
const byUpdated = (a: CatalogEntry, b: CatalogEntry): number => b.updatedAt.localeCompare(a.updatedAt) || byName(a, b);

/**
 * A sorted copy: most recently updated first (ties by name), by name ascending, or by most stars
 * (#119; ties by most recent update then name). Without a stars lookup every mod counts 0 stars,
 * so `stars` falls back to the updated order.
 */
export function sortEntries(
  entries: readonly CatalogEntry[],
  sort: CatalogSort,
  starsOf: (id: string) => number = () => 0,
): CatalogEntry[] {
  const copy = [...entries];
  if (sort === 'name') return copy.sort(byName);
  if (sort === 'stars') return copy.sort((a, b) => starsOf(b.id) - starsOf(a.id) || byUpdated(a, b));
  return copy.sort(byUpdated);
}

/** Lower case without any whitespace, so "Rebalanced  AI" and "rebalancedai" compare equal. */
export function normalizeTitle(title: string): string {
  return title.replace(/\s+/g, '').toLocaleLowerCase();
}

/** True when a library mod has the entry's id or the same normalized title. */
export function inLibrary(entry: CatalogEntry, mods: readonly ModEntry[]): boolean {
  const title = normalizeTitle(entry.title);
  return mods.some((mod) => mod.id === entry.id || normalizeTitle(mod.name) === title);
}

/** The host of the entry's page, shown as text only (the page itself opens by id); `null` if unparsable. */
export function pageHost(entry: CatalogEntry): string | null {
  try {
    return new URL(entry.pageUrl).hostname;
  } catch {
    return null;
  }
}
