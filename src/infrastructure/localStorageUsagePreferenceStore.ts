import type { UsagePreferenceStore } from '../application';

const KEY = 'motogp-launcher.share-usage';

/**
 * Stores "on" / "off" for anonymous usage sharing (#120). An invalid or missing value loads as null
 * (sharing on). A read error propagates: `loadUsagePreference` maps it to "off", so nothing is sent
 * when the choice cannot be remembered. `save` errors are swallowed by the use case.
 */
export function createLocalStorageUsagePreferenceStore(
  storage?: Pick<Storage, 'getItem' | 'setItem'>,
): UsagePreferenceStore {
  // Resolved lazily: reading `window.localStorage` itself can throw (blocked storage).
  const resolve = () => storage ?? window.localStorage;
  return {
    load() {
      const value = resolve().getItem(KEY);
      return value === 'on' || value === 'off' ? value : null;
    },
    save(value) {
      resolve().setItem(KEY, value);
    },
  };
}
