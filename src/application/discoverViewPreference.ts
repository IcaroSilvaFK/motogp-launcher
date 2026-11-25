import type { DiscoverView, DiscoverViewStore } from './ports';

/** The stored view, or the grid when nothing is stored or the store fails. */
export function loadDiscoverView(store: DiscoverViewStore): DiscoverView {
  try {
    return store.load() ?? 'grid';
  } catch {
    return 'grid';
  }
}

/** Persists the view. The adapter may throw; a storage failure must not break the toggle. */
export function saveDiscoverView(store: DiscoverViewStore, view: DiscoverView): void {
  try {
    store.save(view);
  } catch {
    // Ignored on purpose: the view still switches for this session.
  }
}
