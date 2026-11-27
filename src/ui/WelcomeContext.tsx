import { createContext, useContext } from 'react';

export interface WelcomeValue {
  /** Opens the first-run welcome again (Settings > Preferences, #147). */
  show(): void;
}

// Without a welcome store (tests, builds without it) there is nothing to reopen: null.
export const WelcomeContext = createContext<WelcomeValue | null>(null);

export function useWelcome(): WelcomeValue | null {
  return useContext(WelcomeContext);
}
