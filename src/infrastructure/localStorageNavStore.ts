import type { NavPreferenceStore } from '../application';

const KEY = 'motogp-launcher.sidebar-collapsed';

/**
 * Stores "true" / "false". `load` never throws (null on a missing, invalid or unreadable value).
 * `save` lets storage errors propagate; `saveSidebarCollapsed` in the application layer swallows them.
 */
export function createLocalStorageNavStore(storage?: Pick<Storage, 'getItem' | 'setItem'>): NavPreferenceStore {
  // Resolved lazily: reading `window.localStorage` itself can throw (blocked storage).
  const resolve = () => storage ?? window.localStorage;
  return {
    load() {
      try {
        const value = resolve().getItem(KEY);
        if (value === 'true') return true;
        if (value === 'false') return false;
        return null;
      } catch {
        return null;
      }
    },
    save(collapsed) {
      resolve().setItem(KEY, String(collapsed));
    },
  };
}
