import { createContext, useCallback, useContext, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { loadAutoEnablePreference, saveAutoEnablePreference } from '../application';
import type { AutoEnablePreferenceStore } from '../application';

export interface AutoEnablePreferenceValue {
  enabled: boolean;
  setEnabled(value: boolean): void;
}

// Without a provider the preference is on (the default) and cannot be changed.
const AutoEnablePreferenceContext = createContext<AutoEnablePreferenceValue>({
  enabled: true,
  setEnabled: () => undefined,
});

export function AutoEnablePreferenceProvider({
  store,
  children,
}: {
  store?: AutoEnablePreferenceStore;
  children: ReactNode;
}) {
  const [enabled, setState] = useState(() => (store ? loadAutoEnablePreference(store) : true));
  // The save stays out of the state updater, which StrictMode may run twice.
  const setEnabled = useCallback(
    (value: boolean) => {
      setState(value);
      if (store) saveAutoEnablePreference(store, value);
    },
    [store],
  );
  const value = useMemo(() => ({ enabled, setEnabled }), [enabled, setEnabled]);
  return <AutoEnablePreferenceContext.Provider value={value}>{children}</AutoEnablePreferenceContext.Provider>;
}

/** "Enable mods after installing from Discover" and its setter. */
export function useAutoEnablePreference(): AutoEnablePreferenceValue {
  return useContext(AutoEnablePreferenceContext);
}
