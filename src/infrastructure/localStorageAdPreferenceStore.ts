import type { AdPreferenceStore } from '../application';

const KEY = 'motogp-launcher.ad-preference';

/**
 * Stores "on" / "off". An invalid or missing value loads as null. Unlike the other stores, `load`
 * lets a read error propagate (blocked storage): `loadAdPreference` maps it to "off", so ads never
 * show when the choice cannot be remembered. `save` errors propagate and are swallowed by the use case.
 */
export function createLocalStorageAdPreferenceStore(storage?: Pick<Storage, 'getItem' | 'setItem'>): AdPreferenceStore {
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
