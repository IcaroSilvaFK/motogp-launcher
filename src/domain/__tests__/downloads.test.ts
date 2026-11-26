import { describe, expect, it } from 'vitest';
import { DOWNLOAD_DAYS, parseDownloadDaily, parseDownloadTotals, summarizeDaily, usageSharingFor } from '../downloads';

/** 30 UTC days ending 2026-11-26, oldest first, as `get_mod_download_daily` answers. */
function days(counts: (index: number) => number = () => 0) {
  return Array.from({ length: DOWNLOAD_DAYS }, (_, index) => ({
    date: new Date(Date.UTC(2026, 9, 28 + index)).toISOString().slice(0, 10),
    count: counts(index),
  }));
}

describe('parseDownloadTotals', () => {
  it('keeps a well-formed list', () => {
    const totals = [
      { id: 'rebalanced-ai', total: 1234, last30Days: 56 },
      { id: 'tyre-model', total: 0, last30Days: 0 },
    ];
    expect(parseDownloadTotals(totals)).toEqual(totals);
  });

  it('accepts an empty list', () => {
    expect(parseDownloadTotals([])).toEqual([]);
  });

  it('drops unknown fields', () => {
    expect(parseDownloadTotals([{ id: 'a', total: 2, last30Days: 1, extra: true }])).toEqual([
      { id: 'a', total: 2, last30Days: 1 },
    ]);
  });

  it.each([
    ['null (offline)', null],
    ['undefined', undefined],
    ['an object', { id: 'a' }],
    ['a string', 'nope'],
    ['a null item', [null]],
    ['an empty id', [{ id: '', total: 1, last30Days: 0 }]],
    ['a non-string id', [{ id: 7, total: 1, last30Days: 0 }]],
    ['a negative total', [{ id: 'a', total: -1, last30Days: 0 }]],
    ['a fractional total', [{ id: 'a', total: 1.5, last30Days: 0 }]],
    ['a string count', [{ id: 'a', total: '1', last30Days: 0 }]],
    ['a missing last30Days', [{ id: 'a', total: 1 }]],
    ['an infinite count', [{ id: 'a', total: Infinity, last30Days: 0 }]],
  ])('rejects %s', (_, value) => {
    expect(parseDownloadTotals(value)).toBeNull();
  });
});

describe('parseDownloadDaily', () => {
  it('keeps 30 ascending buckets', () => {
    const value = days((index) => index % 3);
    expect(parseDownloadDaily(value)).toEqual(value);
  });

  it.each([
    ['null (offline)', null],
    ['an object', {}],
    ['29 buckets', days().slice(1)],
    ['31 buckets', [...days(), { date: '2026-11-27', count: 0 }]],
    ['a bad date', days().map((day, index) => (index === 4 ? { ...day, date: '2026-13-40' } : day))],
    ['a date with time', days().map((day, index) => (index === 4 ? { ...day, date: `${day.date}T00:00` } : day))],
    ['a negative count', days().map((day, index) => (index === 4 ? { ...day, count: -2 } : day))],
    ['a null bucket', days().map((day, index) => (index === 4 ? null : day))],
    ['descending dates', days().reverse()],
    ['a repeated date', days().map((day, index, all) => (index === 5 ? { ...day, date: all[4]?.date ?? '' } : day))],
  ])('rejects %s', (_, value) => {
    expect(parseDownloadDaily(value)).toBeNull();
  });
});

describe('summarizeDaily', () => {
  it('sums the window and finds the busiest day (the latest one on a tie)', () => {
    const value = days((index) => (index === 3 || index === 20 ? 9 : 1));
    expect(summarizeDaily(value)).toEqual({ total: 46, peak: value[20] });
  });

  it('has no peak when every day is zero', () => {
    expect(summarizeDaily(days())).toEqual({ total: 0, peak: null });
  });
});

describe('usageSharingFor', () => {
  it('shares only when the preference is on', () => {
    expect(usageSharingFor('on')).toBe(true);
    expect(usageSharingFor('off')).toBe(false);
  });
});
