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
