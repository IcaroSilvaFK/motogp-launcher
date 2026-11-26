import { createContext, useContext } from 'react';
import type { CommunityAccount, ModComments, ModStatsStore } from '../application';

/** The community ports wired in `main.tsx` (#122, #119, #118). */
export interface CommunityContextValue {
  /** Absent in tests and builds without the community: no account row renders. */
  account?: CommunityAccount;
  /** Cached mod stars shared by Discover and the mod page (#119); absent: no counts, no star button. */
  stars?: ModStatsStore;
  /** Mod comments (#118). Absent in tests and builds without the community: the section is hidden. */
  comments?: ModComments;
}

export const CommunityContext = createContext<CommunityContextValue>({});

export function useCommunity(): CommunityContextValue {
  return useContext(CommunityContext);
}
