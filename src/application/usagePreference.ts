import type { UsagePreference } from '../domain';
import type { UsagePreferenceStore } from './ports';

/** The stored choice: `on` when nothing is stored, `off` when the store fails (nothing is sent). */
export function loadUsagePreference(store: UsagePreferenceStore): UsagePreference {
  try {
    return store.load() ?? 'on';
  } catch {
    return 'off';
  }
}

/** Persists the choice. The adapter may throw; a storage failure must not break the change. */
export function saveUsagePreference(store: UsagePreferenceStore, value: UsagePreference): void {
  try {
    store.save(value);
  } catch {
    // Ignored on purpose: the choice still applies for this session.
  }
}

/** The live choice, shared by the Settings row and the install report. */
export interface UsagePreferenceHandle {
  get(): UsagePreference;
  set(value: UsagePreference): void;
}

/**
 * Reads the store once and keeps the choice in memory, so an opt-out applies for the whole session
 * even when it cannot be saved.
 */
export function createUsagePreference(store: UsagePreferenceStore): UsagePreferenceHandle {
  let current = loadUsagePreference(store);
  return {
    get: () => current,
    set(value) {
      current = value;
      saveUsagePreference(store, value);
    },
  };
}
