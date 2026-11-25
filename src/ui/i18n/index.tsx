import { createContext, useCallback, useContext, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { en } from './en';
import { DEFAULT_LOCALE, LOCALES } from '../../domain';
import type { Locale } from '../../domain';
import type { Messages } from './messages';
import { ptBR } from './pt-BR';

export type { Messages } from './messages';
export { DEFAULT_LOCALE, LOCALES };
export type { Locale };

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

export function I18nProvider({
  locale: initial = DEFAULT_LOCALE,
  onLocaleChange,
  children,
}: {
  locale?: Locale;
  onLocaleChange?: (locale: Locale) => void;
  children: ReactNode;
}) {
  const [locale, setCurrent] = useState<Locale>(initial);
  const setLocale = useCallback(
    (next: Locale) => {
      setCurrent(next);
      onLocaleChange?.(next);
    },
    [onLocaleChange],
  );
  const value = useMemo<I18n>(() => ({ t: dictionaries[locale], locale, setLocale }), [locale, setLocale]);
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

/** Formats a 0..1 share as a whole percent; a non-zero share under 1% reads "<1%", never "0%". */
export function formatShare(share: number, locale: Locale): string {
  const percent = new Intl.NumberFormat(locale, { style: 'percent', maximumFractionDigits: 0 });
  if (share > 0 && share < 0.01) return `<${percent.format(0.01)}`;
  return percent.format(share);
}

/**
 * Formats Unix ms as a local wall-clock time, hours and minutes with two digits each. The clock is
 * always 24-hour ("14:05"), as racing timetables are; the locale still picks the separator.
 */
export function formatClockTime(ms: number, locale: Locale): string {
  return new Intl.DateTimeFormat(locale, { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(ms);
}
