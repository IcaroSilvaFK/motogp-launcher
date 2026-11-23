import { describe, expect, it } from 'vitest';
import { DEFAULT_LOCALE, isLocale, LOCALES } from '../locale';

describe('locale rules', () => {
  it('defaults to pt-BR and lists both locales', () => {
    expect(DEFAULT_LOCALE).toBe('pt-BR');
    expect(LOCALES).toEqual(['pt-BR', 'en']);
  });

  it('isLocale accepts supported locales only', () => {
    expect(isLocale('pt-BR')).toBe(true);
    expect(isLocale('en')).toBe(true);
    expect(isLocale('fr')).toBe(false);
    expect(isLocale('')).toBe(false);
    expect(isLocale(null)).toBe(false);
    expect(isLocale(undefined)).toBe(false);
    expect(isLocale(42)).toBe(false);
  });
});
