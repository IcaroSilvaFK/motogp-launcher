import { describe, expect, it, vi } from 'vitest';
import { ModStatsError } from '../../domain';
import type { ModStats } from '../../domain';
import type { ModStatsSource } from '../ports/ModStatsSource';
import { createModStatsStore, MOD_STATS_TTL_MS } from '../modStatsStore';

function source(initial: ModStats[] = []) {
  const fake = {
    stats: vi.fn<ModStatsSource['stats']>(() => Promise.resolve(initial)),
    star: vi.fn<ModStatsSource['star']>((id) => Promise.resolve({ id, stars: 1, starredByMe: true })),
    unstar: vi.fn<ModStatsSource['unstar']>((id) => Promise.resolve({ id, stars: 0, starredByMe: false })),
  };
  return fake;
}

function clock(start = 1_000) {
  let now = start;
  return {
    now: () => now,
    advance: (ms: number) => {
      now += ms;
    },
  };
}

describe('createModStatsStore', () => {
  it('starts empty and not loaded', () => {
    const store = createModStatsStore(source());
    expect(store.getSnapshot()).toEqual({ loaded: false, stats: new Map() });
  });

  it('loads the stats into a map keyed by mod id and notifies subscribers', async () => {
    const store = createModStatsStore(source([{ id: 'a', stars: 3 }]));
    const listener = vi.fn();
    store.subscribe(listener);
    await store.load();
    expect(store.getSnapshot().loaded).toBe(true);
    expect(store.getSnapshot().stats.get('a')).toEqual({ id: 'a', stars: 3 });
    expect(listener).toHaveBeenCalled();
  });

  it('keeps the stats for 60 seconds before fetching again', async () => {
    const fake = source([{ id: 'a', stars: 3 }]);
    const time = clock();
    const store = createModStatsStore(fake, { now: time.now });
    await store.load();
    time.advance(MOD_STATS_TTL_MS - 1);
    await store.load();
    expect(fake.stats).toHaveBeenCalledTimes(1);
    time.advance(1);
    await store.load();
    expect(fake.stats).toHaveBeenCalledTimes(2);
  });

  it('shares one request between concurrent loads', async () => {
    const fake = source();
    const store = createModStatsStore(fake);
    await Promise.all([store.load(), store.load()]);
    expect(fake.stats).toHaveBeenCalledTimes(1);
  });

  it('treats a failing source as an empty map without an error', async () => {
    const fake = source();
    fake.stats.mockImplementation(() => Promise.reject(new Error('boom')));
    const store = createModStatsStore(fake);
    await expect(store.load()).resolves.toBeUndefined();
    expect(store.getSnapshot()).toEqual({ loaded: true, stats: new Map() });
  });

  it('treats a throwing source as an empty map', async () => {
    const fake = source();
    fake.stats.mockImplementation(() => {
      throw new Error('sync');
    });
    const store = createModStatsStore(fake);
    await store.load();
    expect(store.getSnapshot()).toEqual({ loaded: true, stats: new Map() });
  });

  it('stars optimistically and then keeps the server answer', async () => {
    const fake = source([{ id: 'a', stars: 10, starredByMe: false }]);
    let answer: (stats: ModStats) => void = () => undefined;
    fake.star.mockImplementation(() => new Promise((resolve) => (answer = resolve)));
    const store = createModStatsStore(fake);
    await store.load();
    const toggled = store.toggleStar('a');
    expect(store.getSnapshot().stats.get('a')).toEqual({ id: 'a', stars: 11, starredByMe: true });
    answer({ id: 'a', stars: 12, starredByMe: true });
    await expect(toggled).resolves.toBe('ok');
    expect(fake.star).toHaveBeenCalledWith('a');
    expect(store.getSnapshot().stats.get('a')).toEqual({ id: 'a', stars: 12, starredByMe: true });
  });

  it('stars a mod that had no stars yet', async () => {
    const fake = source();
    const store = createModStatsStore(fake);
    await store.load();
    await store.toggleStar('fresh');
    expect(fake.star).toHaveBeenCalledWith('fresh');
    expect(store.getSnapshot().stats.get('fresh')).toEqual({ id: 'fresh', stars: 1, starredByMe: true });
  });

  it('unstars a starred mod optimistically', async () => {
    const fake = source([{ id: 'a', stars: 11, starredByMe: true }]);
    fake.unstar.mockResolvedValue({ id: 'a', stars: 10, starredByMe: false });
    const store = createModStatsStore(fake);
    await store.load();
    const toggled = store.toggleStar('a');
    expect(store.getSnapshot().stats.get('a')).toEqual({ id: 'a', stars: 10, starredByMe: false });
    await expect(toggled).resolves.toBe('ok');
    expect(fake.unstar).toHaveBeenCalledWith('a');
  });

  it.each(['signedOut', 'offline', 'failed'] as const)('reverts on a %s failure and reports it', async (kind) => {
    const fake = source([{ id: 'a', stars: 10, starredByMe: false }]);
    fake.star.mockRejectedValue(new ModStatsError(kind));
    const store = createModStatsStore(fake);
    await store.load();
    await expect(store.toggleStar('a')).resolves.toBe(kind);
    expect(store.getSnapshot().stats.get('a')).toEqual({ id: 'a', stars: 10, starredByMe: false });
  });

  it('removes an optimistic entry that did not exist before when it fails', async () => {
    const fake = source();
    fake.star.mockRejectedValue(new Error('unknown'));
    const store = createModStatsStore(fake);
    await store.load();
    await expect(store.toggleStar('fresh')).resolves.toBe('failed');
    expect(store.getSnapshot().stats.has('fresh')).toBe(false);
  });

  it('ignores a second toggle while one is in flight', async () => {
    const fake = source([{ id: 'a', stars: 10 }]);
    fake.star.mockImplementation(() => new Promise(() => undefined));
    const store = createModStatsStore(fake);
    await store.load();
    void store.toggleStar('a');
    await expect(store.toggleStar('a')).resolves.toBe('busy');
    expect(fake.star).toHaveBeenCalledTimes(1);
    expect(store.getSnapshot().stats.get('a')?.stars).toBe(11);
  });

  it('a late stats answer does not undo a star made meanwhile', async () => {
    const fake = source();
    let answer: (stats: ModStats[]) => void = () => undefined;
    fake.stats.mockImplementation(() => new Promise((resolve) => (answer = resolve)));
    const store = createModStatsStore(fake);
    const loading = store.load();
    await store.toggleStar('a');
    answer([
      { id: 'a', stars: 0 },
      { id: 'b', stars: 2 },
    ]);
    await loading;
    expect(store.getSnapshot().stats.get('a')).toEqual({ id: 'a', stars: 1, starredByMe: true });
    expect(store.getSnapshot().stats.get('b')?.stars).toBe(2);
  });

  it('stops notifying after unsubscribe', async () => {
    const store = createModStatsStore(source());
    const listener = vi.fn();
    const unsubscribe = store.subscribe(listener);
    unsubscribe();
    await store.load();
    expect(listener).not.toHaveBeenCalled();
  });

  it('invalidate makes the next load fetch again', async () => {
    const fake = source();
    const store = createModStatsStore(fake);
    await store.load();
    store.invalidate();
    await store.load();
    expect(fake.stats).toHaveBeenCalledTimes(2);
  });
});
