import { describe, expect, it } from 'vitest';
import { adsEnabledFor, shouldPromptForAds } from '../ads';

describe('adsEnabledFor', () => {
  it.each([
    ['on', true],
    ['unset', true],
    ['off', false],
  ] as const)('returns the default for %j', (preference, expected) => {
    expect(adsEnabledFor(preference)).toBe(expected);
  });
});

describe('shouldPromptForAds', () => {
  it.each([
    ['unset', true],
    ['on', false],
    ['off', false],
  ] as const)('returns whether to prompt for %j', (preference, expected) => {
    expect(shouldPromptForAds(preference)).toBe(expected);
  });
});
