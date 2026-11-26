import type { AdSlotId, AdViewModel } from '../../domain';

/**
 * Sponsored creatives (#104). The backend owns the feed, the image fetch and the click URL; the UI
 * only sends a slot and a creative id. Implementations resolve `null` for "no ad" (none, offline,
 * a missing command or a malformed answer) and never reject from `click` / `reportShown`.
 */
export interface AdProvider {
  get(slot: AdSlotId): Promise<AdViewModel | null>;
  /** Opens the creative's page in the browser (the backend holds the URL). */
  click(slot: AdSlotId, creativeId: string): Promise<void>;
  /** Called once the card is actually on screen. */
  reportShown(slot: AdSlotId, creativeId: string): Promise<void>;
}
