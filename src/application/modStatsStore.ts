import { useEffect, useSyncExternalStore } from 'react';
import { toModStatsError } from '../domain';
import type { ModStats, ModStatsErrorKind } from '../domain';
import type { ModStatsSource } from './ports/ModStatsSource';

/** How long one stats answer is reused before the next `load()` asks again (#119). */
export const MOD_STATS_TTL_MS = 60_000;

export interface ModStatsSnapshot {
  /** False until the first stats answer (or failure) arrived. */
  loaded: boolean;
  /** Mods with stars, by catalog id. A mod missing here has 0 stars; empty when offline. */
  stats: ReadonlyMap<string, ModStats>;
}

/** `ok`, a typed failure (the change was reverted), or `busy` when a change for that mod is in flight. */
export type ToggleStarResult = 'ok' | 'busy' | ModStatsErrorKind;

export interface ModStatsStore {
  getSnapshot(): ModStatsSnapshot;
  subscribe(listener: () => void): () => void;
  /** Fetches the stats unless an answer younger than the TTL is cached. Never rejects. */
  load(): Promise<void>;
  /** Forgets the cache age, so the next `load()` fetches again (e.g. after signing in). */
  invalidate(): void;
  /** Stars or unstars the mod at once and reverts when the server refuses. Never rejects. */
  toggleStar(id: string): Promise<ToggleStarResult>;
}

/**
 * Cached community stats shared by Discover and the mod page (#119): one request per TTL,
 * any failure is an empty map (no error), and star toggles are optimistic.
 */
export function createModStatsStore(
  source: ModStatsSource,
  { now = Date.now, ttlMs = MOD_STATS_TTL_MS }: { now?: () => number; ttlMs?: number } = {},
): ModStatsStore {
  let snapshot: ModStatsSnapshot = { loaded: false, stats: new Map() };
  let fetchedAt: number | null = null;
  let inFlight: Promise<void> | null = null;
  const pending = new Set<string>();
  /** Mods changed locally while a stats request was out: their local value wins over its answer. */
  const touched = new Set<string>();
  const listeners = new Set<() => void>();

  const publish = (next: ModStatsSnapshot) => {
    snapshot = next;
    for (const listener of listeners) listener();
  };

  const put = (id: string, value: ModStats | undefined) => {
    const stats = new Map(snapshot.stats);
    if (value === undefined) stats.delete(id);
    else stats.set(id, value);
    publish({ ...snapshot, stats });
  };

  const fetchStats = async () => {
    touched.clear();
    for (const id of pending) touched.add(id);
    let list: ModStats[];
    try {
      list = await source.stats();
    } catch {
      list = [];
    }
    const stats = new Map(list.map((item) => [item.id, item]));
    for (const id of touched) {
      const local = snapshot.stats.get(id);
      if (local === undefined) stats.delete(id);
      else stats.set(id, local);
    }
    fetchedAt = now();
    publish({ loaded: true, stats });
  };

  return {
    getSnapshot: () => snapshot,
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    load() {
      if (inFlight !== null) return inFlight;
      if (fetchedAt !== null && now() - fetchedAt < ttlMs) return Promise.resolve();
      inFlight = fetchStats().finally(() => {
        inFlight = null;
      });
      return inFlight;
    },
    invalidate() {
      fetchedAt = null;
    },
    async toggleStar(id) {
      if (pending.has(id)) return 'busy';
      pending.add(id);
      touched.add(id);
      const before = snapshot.stats.get(id);
      const starred = before?.starredByMe === true;
      const stars = before?.stars ?? 0;
      put(id, { id, stars: starred ? Math.max(0, stars - 1) : stars + 1, starredByMe: !starred });
      try {
        const answer = await (starred ? source.unstar(id) : source.star(id));
        put(id, answer);
        return 'ok';
      } catch (error) {
        put(id, before);
        return toModStatsError(error).kind;
      } finally {
        pending.delete(id);
      }
    },
  };
}

const EMPTY: ModStatsSnapshot = { loaded: false, stats: new Map() };
const noop = () => () => undefined;

/**
 * The store's snapshot, loading it on mount (cached per TTL). Without a store the snapshot stays
 * empty and not loaded, so callers render nothing community-related.
 */
export function useModStats(store: ModStatsStore | undefined): ModStatsSnapshot {
  const snapshot = useSyncExternalStore(store ? store.subscribe : noop, store ? store.getSnapshot : () => EMPTY);
  useEffect(() => {
    void store?.load();
  }, [store]);
  return snapshot;
}
