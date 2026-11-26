import { createContext, useContext } from 'react';
import type { CommunityAccount, ModComments, ModPublisher, ModStatsStore, PublishFilePicker } from '../application';

/** The community ports wired in `main.tsx` (#122, #119, #118). */
export interface CommunityContextValue {
  /** Absent in tests and builds without the community: no account row renders. */
  account?: CommunityAccount;
  /** Cached mod stars shared by Discover and the mod page (#119); absent: no counts, no star button. */
  stars?: ModStatsStore;
  /** Mod comments (#118). Absent in tests and builds without the community: the section is hidden. */
  comments?: ModComments;
  /** Mod hosting (#130): publish a mod and list my mods. Absent: no publish UI. */
  publisher?: ModPublisher;
  /** Native dialogs of the publish form (#130). */
  publishPicker?: PublishFilePicker;
}

export const CommunityContext = createContext<CommunityContextValue>({});

export function useCommunity(): CommunityContextValue {
  return useContext(CommunityContext);
}
