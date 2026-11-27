import type { LibrarySortStore } from '../application';

const KEY = 'motogp-launcher.library-sort';

/**
 * Stores "name" / "size". `load` never throws (null on a missing, invalid or unreadable value).
 * `save` lets storage errors propagate; `saveLibrarySort` in the application layer swallows them.
 */
export function createLocalStorageLibrarySortStore(storage?: Pick<Storage, 'getItem' | 'setItem'>): LibrarySortStore {
  // Resolved lazily: reading `window.localStorage` itself can throw (blocked storage).
  const resolve = () => storage ?? window.localStorage;
  return {
    load() {
      try {
        const value = resolve().getItem(KEY);
        return value === 'name' || value === 'size' ? value : null;
      } catch {
        return null;
      }
    },
    save(sort) {
      resolve().setItem(KEY, sort);
    },
  };
}
