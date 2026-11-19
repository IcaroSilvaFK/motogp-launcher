import { createContext, useContext, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { en } from './en';
import type { Locale, Messages } from './messages';
import { ptBR } from './pt-BR';

export type { Locale, Messages } from './messages';

export const DEFAULT_LOCALE: Locale = 'pt-BR';
export const LOCALES: readonly Locale[] = ['pt-BR', 'en'];

const dictionaries: Record<Locale, Messages> = { 'pt-BR': ptBR, en };

export interface I18n {
  t: Messages;
  locale: Locale;
  setLocale: (locale: Locale) => void;
}

const I18nContext = createContext<I18n>({
  t: dictionaries[DEFAULT_LOCALE],
  locale: DEFAULT_LOCALE,
  setLocale: () => undefined,
});

export function I18nProvider({ locale: initial = DEFAULT_LOCALE, children }: { locale?: Locale; children: ReactNode }) {
  const [locale, setLocale] = useState<Locale>(initial);
  const value = useMemo<I18n>(() => ({ t: dictionaries[locale], locale, setLocale }), [locale]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18n {
  return useContext(I18nContext);
}

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const;

/** Formats a byte count with binary steps (1 KB = 1024 B) and locale-aware decimals. */
export function formatSize(bytes: number, locale: Locale): string {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const number = new Intl.NumberFormat(locale, { maximumFractionDigits: unit === 0 ? 0 : 1 }).format(value);
  return `${number} ${UNITS[unit]}`;
}
