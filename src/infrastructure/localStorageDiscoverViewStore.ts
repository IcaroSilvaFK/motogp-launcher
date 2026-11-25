import type { DiscoverViewStore } from '../application';

const KEY = 'motogp-launcher.discover-view';

/**
 * Stores "grid" / "list". `load` never throws (null on a missing, invalid or unreadable value).
 * `save` lets storage errors propagate; `saveDiscoverView` in the application layer swallows them.
 */
export function createLocalStorageDiscoverViewStore(storage?: Pick<Storage, 'getItem' | 'setItem'>): DiscoverViewStore {
  // Resolved lazily: reading `window.localStorage` itself can throw (blocked storage).
  const resolve = () => storage ?? window.localStorage;
  return {
    load() {
      try {
        const value = resolve().getItem(KEY);
        return value === 'grid' || value === 'list' ? value : null;
      } catch {
        return null;
      }
    },
    save(view) {
      resolve().setItem(KEY, view);
    },
  };
}
