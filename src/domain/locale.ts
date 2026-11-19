export type Locale = 'pt-BR' | 'en';

export const LOCALES: readonly Locale[] = ['pt-BR', 'en'];

export const DEFAULT_LOCALE: Locale = 'pt-BR';

export function isLocale(value: unknown): value is Locale {
  return LOCALES.some((locale) => locale === value);
}
