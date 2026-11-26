/** The user's sponsored-content choice. `unset` means they never chose. */
export type AdPreference = 'on' | 'off' | 'unset';

/** Sponsored cards (self-hosted feed) are on by default, so an unset choice shows them. */
export function adsEnabledFor(preference: AdPreference): boolean {
  return preference !== 'off';
}

/** Only a user who never chose sees the first-launch prompt. */
export function shouldPromptForAds(preference: AdPreference): boolean {
  return preference === 'unset';
}

// Sponsored placements (#104). The backend fetches the creative and returns it over IPC; the image
// is a `data:` URI (CSP `img-src data:`) and the click URL never reaches the UI.

/** Where a sponsored card can appear. */
export type AdSlotId = 'discover' | 'sidebar' | 'about';

export const AD_SLOTS: readonly AdSlotId[] = ['discover', 'sidebar', 'about'];

/** One creative as the UI shows it (camelCase, as serialized by `get_ad`). */
export interface AdViewModel {
  id: string;
  title: string;
  body: string;
  /** A raster `data:image/...;base64,` URI; SVG and remote URLs are refused. */
  imageDataUri: string;
  /** The sponsor's own label, shown next to the launcher's "Sponsored" label. */
  sponsorLabel: string;
}

const RASTER_DATA_URI = /^data:image\/(png|jpeg|webp|gif);base64,[A-Za-z0-9+/]+=*$/;

function text(value: unknown): value is string {
  return typeof value === 'string' && value.trim() !== '';
}

/**
 * Validates an untrusted `get_ad` payload: `null` for anything that is not a complete creative with
 * a raster `data:` image, so a malformed answer behaves exactly like "no ad".
 */
export function toAdViewModel(value: unknown): AdViewModel | null {
  if (typeof value !== 'object' || value === null) return null;
  const { id, title, body, imageDataUri, sponsorLabel } = value as Record<string, unknown>;
  if (!text(id) || !text(title) || typeof body !== 'string' || typeof sponsorLabel !== 'string') return null;
  if (typeof imageDataUri !== 'string' || !RASTER_DATA_URI.test(imageDataUri)) return null;
  return { id, title, body, imageDataUri, sponsorLabel };
}
