import { describe, expect, it } from 'vitest';
import type { CatalogEntry, ModEntry } from '../../domain';
import { categoryCounts, inLibrary, normalizeTitle, pageHost, selectEntries, sortEntries } from '../catalogSelectors';

function entry(patch: Partial<CatalogEntry> & { id: string }): CatalogEntry {
  return {
    title: patch.id,
    author: 'Someone',
    game: 'MotoGP 25',
    category: 'Physics',
    description: '',
    version: '1.0',
    updatedAt: '2026-01-01',
    pageUrl: 'https://overtake.gg/downloads/x/',
    thumbnailUrl: null,
    screenshots: [],
    longDescription: null,
    download: null,
    ...patch,
  };
}

const ai = entry({ id: 'rebalanced-ai', title: 'Rebalanced AI', author: 'Ana', updatedAt: '2026-11-10' });
const tyres = entry({ id: 'tyre-model', title: 'Tyre Model', author: 'Rebal Works', updatedAt: '2026-11-20' });
const livery = entry({ id: 'yamaha', title: 'Yamaha Livery', category: 'Liveries', updatedAt: '2026-10-01' });
const all = [ai, tyres, livery];

const mod = (id: string, name: string): ModEntry => ({ id, name, files: [], size_bytes: 0 });

describe('categoryCounts', () => {
  it('counts entries per category, sorted by name', () => {
    expect(categoryCounts(all)).toEqual([
      { category: 'Liveries', count: 1 },
      { category: 'Physics', count: 2 },
    ]);
  });

  it('is empty without entries', () => {
    expect(categoryCounts([])).toEqual([]);
  });
});

describe('selectEntries', () => {
  it('keeps everything without a category or a query', () => {
    expect(selectEntries(all, { category: null, query: '  ' })).toEqual(all);
  });

  it('filters by category', () => {
    expect(selectEntries(all, { category: 'Physics', query: '' }).map((e) => e.id)).toEqual([
      'rebalanced-ai',
      'tyre-model',
    ]);
  });

  it('matches the query against title, author or category, case-insensitive', () => {
    expect(selectEntries(all, { category: null, query: 'REBAL' }).map((e) => e.id)).toEqual([
      'rebalanced-ai',
      'tyre-model',
    ]);
    expect(selectEntries(all, { category: null, query: 'liver' }).map((e) => e.id)).toEqual(['yamaha']);
    expect(selectEntries(all, { category: 'Liveries', query: 'rebal' })).toEqual([]);
  });
});

describe('sortEntries', () => {
  it('sorts by most recently updated first without mutating the input', () => {
    const input = [...all];
    expect(sortEntries(input, 'updated').map((e) => e.id)).toEqual(['tyre-model', 'rebalanced-ai', 'yamaha']);
    expect(input).toEqual(all);
  });

  it('breaks update ties by name', () => {
    const b = entry({ id: 'b', title: 'Bravo', updatedAt: '2026-01-01' });
    const a = entry({ id: 'a', title: 'alpha', updatedAt: '2026-01-01' });
    expect(sortEntries([b, a], 'updated').map((e) => e.id)).toEqual(['a', 'b']);
  });

  it('sorts by name ascending, case-insensitive', () => {
    const lower = entry({ id: 'z', title: 'apex Sound' });
    expect(sortEntries([...all, lower], 'name').map((e) => e.title)).toEqual([
      'apex Sound',
      'Rebalanced AI',
      'Tyre Model',
      'Yamaha Livery',
    ]);
  });

  it('sorts by most stars, ties by most recent update then name (#119)', () => {
    const stars: Record<string, number> = { yamaha: 12, 'rebalanced-ai': 5 };
    const starsOf = (id: string) => stars[id] ?? 0;
    expect(sortEntries(all, 'stars', starsOf).map((e) => e.id)).toEqual(['yamaha', 'rebalanced-ai', 'tyre-model']);
    const twin = entry({ id: 'aaa', title: 'Aaa', updatedAt: '2026-11-10' });
    const tied = { yamaha: 3, 'rebalanced-ai': 3, aaa: 3, 'tyre-model': 3 } as Record<string, number>;
    expect(sortEntries([...all, twin], 'stars', (id) => tied[id] ?? 0).map((e) => e.id)).toEqual([
      'tyre-model',
      'aaa',
      'rebalanced-ai',
      'yamaha',
    ]);
  });

  it('falls back to the updated order when sorting by stars without counts', () => {
    expect(sortEntries(all, 'stars').map((e) => e.id)).toEqual(sortEntries(all, 'updated').map((e) => e.id));
  });
});

describe('inLibrary', () => {
  it('matches the same id', () => {
    expect(inLibrary(ai, [mod('rebalanced-ai', 'Something else')])).toBe(true);
  });

  it('matches the title ignoring case and whitespace', () => {
    expect(inLibrary(ai, [mod('m1', '  rebalanced   ai ')])).toBe(true);
    expect(inLibrary(ai, [mod('m1', 'RebalancedAI')])).toBe(true);
  });

  it('is false when nothing matches', () => {
    expect(inLibrary(ai, [mod('m1', 'Rebalanced AI v2')])).toBe(false);
    expect(inLibrary(ai, [])).toBe(false);
  });

  it('normalizes titles', () => {
    expect(normalizeTitle(' A  b\tC ')).toBe('abc');
  });
});

describe('pageHost', () => {
  it('returns the host of the page', () => {
    expect(pageHost(ai)).toBe('overtake.gg');
  });

  it('returns null for an unparsable URL', () => {
    expect(pageHost(entry({ id: 'x', pageUrl: 'not a url' }))).toBeNull();
  });
});
