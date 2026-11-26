import type { AutoEnablePreferenceStore } from '../application';

const KEY = 'motogp-launcher.auto-enable-after-install';

/** Stores "on" / "off". An invalid or missing value loads as null (the default, on). Errors propagate to the use case. */
export function createLocalStorageAutoEnableStore(
  storage?: Pick<Storage, 'getItem' | 'setItem'>,
): AutoEnablePreferenceStore {
  const resolve = () => storage ?? window.localStorage;
  return {
    load() {
      const value = resolve().getItem(KEY);
      return value === 'on' ? true : value === 'off' ? false : null;
    },
    save(value) {
      resolve().setItem(KEY, value ? 'on' : 'off');
    },
  };
}
