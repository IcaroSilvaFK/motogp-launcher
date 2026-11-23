import type { NavPreferenceStore } from './ports';

/** The stored choice, or expanded (false) when nothing is stored or the store fails. */
export function loadSidebarCollapsed(store: NavPreferenceStore): boolean {
  try {
    return store.load() ?? false;
  } catch {
    return false;
  }
}

/** Persists the choice. The adapter may throw; a storage failure must not break the toggle. */
export function saveSidebarCollapsed(store: NavPreferenceStore, collapsed: boolean): void {
  try {
    store.save(collapsed);
  } catch {
    // Ignored on purpose: the sidebar still toggles for this session.
  }
}
