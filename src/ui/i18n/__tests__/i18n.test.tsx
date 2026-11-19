import { act, render, renderHook, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ERROR_CODES } from '../../../domain';
import { en } from '../en';
import { DEFAULT_LOCALE, formatSize, I18nProvider, useI18n } from '../index';
import { ptBR } from '../pt-BR';

function keysOf(value: object, prefix = ''): string[] {
  return Object.entries(value).flatMap(([key, child]) =>
    child !== null && typeof child === 'object' ? keysOf(child, `${prefix}${key}.`) : [`${prefix}${key}`],
  );
}

describe('i18n dictionaries', () => {
  it('have the same keys in both locales', () => {
    expect(keysOf(ptBR).sort()).toEqual(keysOf(en).sort());
  });

  it('have a message for every error code in both locales', () => {
    for (const code of ERROR_CODES) {
      expect(en.errors[code], code).toBeTruthy();
      expect(ptBR.errors[code], code).toBeTruthy();
    }
  });

  it('keeps the exact english session texts', () => {
    expect(en.session.running).toBe('Running…');
    expect(en.session.ended(2)).toBe('Session ended — 2 files removed');
    expect(en.session.neverStarted).toBe('The game did not open within 2 min; mods removed');
    expect(en.settings.useThis).toBe('Use this');
    expect(en.settings.chooseFolder).toBe('Choose folder…');
    expect(en.settings.appIdNotFound).toBe('not found');
  });

  it('has specific messages for the carried-over codes', () => {
    for (const code of [
      'game_still_running',
      'process_inspection_failed',
      'session_thread_failed',
      'archive_too_large',
    ] as const) {
      expect(en.errors[code]).not.toBe(en.errors.unknown);
      expect(ptBR.errors[code]).not.toBe(ptBR.errors.unknown);
    }
  });
});

describe('I18nProvider', () => {
  it('defaults to pt-BR', () => {
    expect(DEFAULT_LOCALE).toBe('pt-BR');
    const { result } = renderHook(() => useI18n(), { wrapper: I18nProvider });
    expect(result.current.locale).toBe('pt-BR');
    expect(result.current.t).toBe(ptBR);
  });

  it('uses the given initial locale and switches with setLocale', () => {
    const { result } = renderHook(() => useI18n(), {
      wrapper: ({ children }) => <I18nProvider locale="en">{children}</I18nProvider>,
    });
    expect(result.current.t).toBe(en);
    act(() => result.current.setLocale('pt-BR'));
    expect(result.current.locale).toBe('pt-BR');
    expect(result.current.t).toBe(ptBR);
  });

  it('renders children', () => {
    render(
      <I18nProvider locale="en">
        <p>child</p>
      </I18nProvider>,
    );
    expect(screen.getByText('child')).toBeInTheDocument();
  });

  it('falls back to the default locale outside a provider', () => {
    const { result } = renderHook(() => useI18n());
    expect(result.current.locale).toBe('pt-BR');
  });
});

describe('formatSize', () => {
  it('formats bytes with locale-aware units', () => {
    expect(formatSize(0, 'en')).toBe('0 B');
    expect(formatSize(512, 'en')).toBe('512 B');
    expect(formatSize(1536, 'en')).toBe('1.5 KB');
    expect(formatSize(5 * 1024 * 1024, 'en')).toBe('5 MB');
    expect(formatSize(1.5 * 1024 * 1024 * 1024, 'pt-BR')).toBe('1,5 GB');
  });
});
