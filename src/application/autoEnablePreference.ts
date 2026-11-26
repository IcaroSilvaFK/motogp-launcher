import type { AutoEnablePreferenceStore } from './ports';

/** The stored choice; on by default, and when nothing is stored or the store fails. */
export function loadAutoEnablePreference(store: AutoEnablePreferenceStore): boolean {
  try {
    return store.load() ?? true;
  } catch {
    return true;
  }
}

/** Persists the choice. The adapter may throw; a storage failure must not break the change. */
export function saveAutoEnablePreference(store: AutoEnablePreferenceStore, value: boolean): void {
  try {
    store.save(value);
  } catch {
    // Ignored on purpose: the choice still applies for this session.
  }
}
