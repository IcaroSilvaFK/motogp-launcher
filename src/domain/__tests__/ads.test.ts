import { describe, expect, it } from 'vitest';
import { AD_SLOTS, adsEnabledFor, shouldPromptForAds, toAdViewModel } from '../ads';

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

const valid = {
  id: 'house-apoiase',
  title: 'Support the launcher',
  body: 'Keep it free.',
  imageDataUri: 'data:image/png;base64,iVBORw0KGgo=',
  sponsorLabel: 'MotoGP Launcher',
};

describe('toAdViewModel', () => {
  it('lists the three slots', () => {
    expect(AD_SLOTS).toEqual(['discover', 'sidebar', 'about']);
  });

  it('keeps a complete creative and drops extra fields such as a click URL', () => {
    expect(toAdViewModel({ ...valid, clickUrl: 'https://example.com' })).toEqual(valid);
  });

  it('accepts an empty body and an empty sponsor label', () => {
    expect(toAdViewModel({ ...valid, body: '', sponsorLabel: '' })).toEqual({ ...valid, body: '', sponsorLabel: '' });
  });

  it.each([
    ['null', null],
    ['a string', 'ad'],
    ['a number', 42],
    ['no id', { ...valid, id: undefined }],
    ['a blank id', { ...valid, id: '  ' }],
    ['a blank title', { ...valid, title: '' }],
    ['a non-string body', { ...valid, body: 3 }],
    ['a missing sponsor label', { ...valid, sponsorLabel: null }],
    ['a missing image', { ...valid, imageDataUri: undefined }],
    ['a remote image URL', { ...valid, imageDataUri: 'https://ads.example.com/a.png' }],
    ['an SVG data URI', { ...valid, imageDataUri: 'data:image/svg+xml;base64,PHN2Zz4=' }],
    ['a non-base64 data URI', { ...valid, imageDataUri: 'data:image/png,<script>' }],
  ])('rejects %s', (_name, payload) => {
    expect(toAdViewModel(payload)).toBeNull();
  });
});
