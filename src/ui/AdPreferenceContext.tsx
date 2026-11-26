import { createContext, useCallback, useContext, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { loadAdPreference, saveAdPreference } from '../application';
import type { AdPreferenceStore } from '../application';
import { adsEnabledFor } from '../domain';
import type { AdPreference } from '../domain';

export interface AdPreferenceValue {
  preference: AdPreference;
  setPreference(value: 'on' | 'off'): void;
}

// Without a provider (or a store) the preference is "off": no prompt and no ads.
const AdPreferenceContext = createContext<AdPreferenceValue>({ preference: 'off', setPreference: () => undefined });

export function AdPreferenceProvider({ store, children }: { store?: AdPreferenceStore; children: ReactNode }) {
  // Read once on mount; without a store the preference is off and lives in memory.
  const [preference, setState] = useState<AdPreference>(() => (store ? loadAdPreference(store) : 'off'));
  // The save stays out of the state updater, which StrictMode may run twice.
  const setPreference = useCallback(
    (value: 'on' | 'off') => {
      setState(value);
      if (store) saveAdPreference(store, value);
    },
    [store],
  );
  const value = useMemo(() => ({ preference, setPreference }), [preference, setPreference]);
  return <AdPreferenceContext.Provider value={value}>{children}</AdPreferenceContext.Provider>;
}

/** The stored choice and its setter. */
export function useAdPreference(): AdPreferenceValue {
  return useContext(AdPreferenceContext);
}

/** Whether sponsored content may render; every `AdSlot` reads it before requesting a creative. */
export function useAdsEnabled(): boolean {
  return adsEnabledFor(useAdPreference().preference);
}
