import type { ModStats } from '../../domain';

/**
 * Community stars of the mods (#119). The backend holds the session token and talks to the
 * community server; the UI only sees the numbers.
 */
export interface ModStatsSource {
  /** Stats of every mod that has stars. Never rejects: any failure (offline, bad answer) is `[]`. */
  stats(): Promise<ModStats[]>;
  /** Stars a mod. Rejects with a `ModStatsError` (`signedOut`, `offline` or `failed`). */
  star(id: string): Promise<ModStats>;
  /** Removes the star. Rejects with a `ModStatsError` (`signedOut`, `offline` or `failed`). */
  unstar(id: string): Promise<ModStats>;
}
