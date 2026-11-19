import { describe, expect, it } from 'vitest';
import type { Locale } from '../../domain';
import { loadLocale, saveLocale } from '../localePreference';
import type { LocaleStore } from '../ports';

function fakeStore(initial: Locale | null = null): LocaleStore & { value: Locale | null } {
  const store = {
    value: initial,
    load: () => store.value,
    save: (locale: Locale) => {
      store.value = locale;
    },
  };
  return store;
}

const throwingStore: LocaleStore = {
  load: () => {
    throw new Error('storage unavailable');
  },
  save: () => {
    throw new Error('storage unavailable');
  },
};

describe('loadLocale', () => {
  it('returns the stored locale', () => {
    expect(loadLocale(fakeStore('en'))).toBe('en');
  });

  it('falls back to pt-BR when nothing is stored', () => {
    expect(loadLocale(fakeStore(null))).toBe('pt-BR');
  });

  it('falls back to pt-BR when the store throws', () => {
    expect(loadLocale(throwingStore)).toBe('pt-BR');
  });
});

describe('saveLocale', () => {
  it('saves the locale', () => {
    const store = fakeStore();
    saveLocale(store, 'en');
    expect(store.value).toBe('en');
  });

  it('never throws when the store fails', () => {
    expect(() => saveLocale(throwingStore, 'en')).not.toThrow();
  });
});
