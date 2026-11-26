import { describe, expect, it } from 'vitest';
import { loadAdPreference, saveAdPreference } from '../adPreference';
import type { AdPreferenceStore } from '../ports';

function fakeStore(initial: 'on' | 'off' | null = null): AdPreferenceStore & { value: 'on' | 'off' | null } {
  const store = {
    value: initial,
    load: () => store.value,
    save: (value: 'on' | 'off') => {
      store.value = value;
    },
  };
  return store;
}

const throwingStore: AdPreferenceStore = {
  load: () => {
    throw new Error('storage unavailable');
  },
  save: () => {
    throw new Error('storage unavailable');
  },
};

describe('loadAdPreference', () => {
  it('is unset when nothing is stored', () => {
    expect(loadAdPreference(fakeStore(null))).toBe('unset');
  });

  it.each(['on', 'off'] as const)('returns the stored value %j', (value) => {
    expect(loadAdPreference(fakeStore(value))).toBe(value);
  });

  it('is off when the store throws', () => {
    expect(loadAdPreference(throwingStore)).toBe('off');
  });
});

describe('saveAdPreference', () => {
  it('persists the value', () => {
    const store = fakeStore();
    saveAdPreference(store, 'off');
    expect(store.value).toBe('off');
  });

  it('swallows store errors', () => {
    expect(() => saveAdPreference(throwingStore, 'on')).not.toThrow();
  });
});
