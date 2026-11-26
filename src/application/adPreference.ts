import type { AdPreference } from '../domain';
import type { AdPreferenceStore } from './ports';

/** The stored choice: `unset` when nothing is stored, `off` when the store fails (no prompt, no ads). */
export function loadAdPreference(store: AdPreferenceStore): AdPreference {
  try {
    return store.load() ?? 'unset';
  } catch {
    return 'off';
  }
}

/** Persists the choice. The adapter may throw; a storage failure must not break the change. */
export function saveAdPreference(store: AdPreferenceStore, value: 'on' | 'off'): void {
  try {
    store.save(value);
  } catch {
    // Ignored on purpose: the choice still applies for this session.
  }
}
