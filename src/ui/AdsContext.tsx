import { createContext, useContext } from 'react';
import type { AdProvider } from '../application';

/**
 * Sponsored placements (#104): the provider wired in `main.tsx`. Whether ads may show comes from the
 * user's preference (`useAdsEnabled`, #105), not from here.
 */
export interface AdsContextValue {
  /** Absent in tests and builds without ads: no slot renders and nothing is requested. */
  provider?: AdProvider;
}

export const AdsContext = createContext<AdsContextValue>({});

export function useAds(): AdsContextValue {
  return useContext(AdsContext);
}
