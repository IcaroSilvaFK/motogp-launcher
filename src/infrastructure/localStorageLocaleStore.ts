import { isLocale } from '../domain';
import type { LocaleStore } from '../application';

const KEY = 'motogp-launcher.locale';

/**
 * `load` never throws (null on a missing, invalid or unreadable value). `save` lets storage
 * errors propagate; `saveLocale` in the application layer is what swallows them.
 */
export function createLocalStorageLocaleStore(storage?: Pick<Storage, 'getItem' | 'setItem'>): LocaleStore {
  // Resolved lazily: reading `window.localStorage` itself can throw (blocked storage).
  const resolve = () => storage ?? window.localStorage;
  return {
    load() {
      try {
        const value = resolve().getItem(KEY);
        return isLocale(value) ? value : null;
      } catch {
        return null;
      }
    },
    save(locale) {
      resolve().setItem(KEY, locale);
    },
  };
}
