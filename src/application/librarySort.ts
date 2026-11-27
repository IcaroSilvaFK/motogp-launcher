import type { LibrarySort, LibrarySortStore } from './ports';

/** The stored sort, or name (A-Z) when nothing is stored or the store fails. */
export function loadLibrarySort(store: LibrarySortStore): LibrarySort {
  try {
    return store.load() ?? 'name';
  } catch {
    return 'name';
  }
}

/** Persists the sort. The adapter may throw; a storage failure must not break the control. */
export function saveLibrarySort(store: LibrarySortStore, sort: LibrarySort): void {
  try {
    store.save(sort);
  } catch {
    // Ignored on purpose: the order still applies for this session.
  }
}
