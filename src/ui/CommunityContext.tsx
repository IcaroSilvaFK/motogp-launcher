import { createContext, useContext } from 'react';
import type { CommunityAccount } from '../application';

/** The community account port wired in `main.tsx` (#122). */
export interface CommunityContextValue {
  /** Absent in tests and builds without the community: no account row renders. */
  account?: CommunityAccount;
}

export const CommunityContext = createContext<CommunityContextValue>({});

export function useCommunity(): CommunityContextValue {
  return useContext(CommunityContext);
}
