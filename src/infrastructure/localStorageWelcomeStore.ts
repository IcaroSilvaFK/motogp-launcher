import type { WelcomeStore } from '../application';

const KEY = 'motogp-launcher.welcome-seen';

/**
 * Stores "true" / "false". `load` returns null on a missing or invalid value and lets a read
 * failure throw, so `loadWelcomeSeen` treats blocked storage as seen instead of showing the
 * welcome on every launch. `save` lets storage errors propagate; `saveWelcomeSeen` swallows them.
 */
export function createLocalStorageWelcomeStore(storage?: Pick<Storage, 'getItem' | 'setItem'>): WelcomeStore {
  // Resolved lazily: reading `window.localStorage` itself can throw (blocked storage).
  const resolve = () => storage ?? window.localStorage;
  return {
    load() {
      const value = resolve().getItem(KEY);
      if (value === 'true') return true;
      if (value === 'false') return false;
      return null;
    },
    save(seen) {
      resolve().setItem(KEY, String(seen));
    },
  };
}
