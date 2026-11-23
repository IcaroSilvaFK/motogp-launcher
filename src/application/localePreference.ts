import { DEFAULT_LOCALE } from '../domain';
import type { Locale } from '../domain';
import type { LocaleStore } from './ports';

/** The stored locale, or the default one when nothing is stored or the store fails. */
export function loadLocale(store: LocaleStore): Locale {
  try {
    return store.load() ?? DEFAULT_LOCALE;
  } catch {
    return DEFAULT_LOCALE;
  }
}

/** Persists the choice. The adapter may throw; a storage failure must not break the language switch. */
export function saveLocale(store: LocaleStore, locale: Locale): void {
  try {
    store.save(locale);
  } catch {
    // Ignored on purpose: the language still changes for this session.
  }
}
