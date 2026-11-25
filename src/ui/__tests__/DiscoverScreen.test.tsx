// DiscoverScreen details outside the Gherkin scenarios: the #74 Install slot, refresh failures,
// the sheet's focus trap and scrim, and the pt-BR copy.
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createCatalogController } from '../../application';
import type { CatalogGateway } from '../../application';
import type { Catalog, CatalogEntry, CommandError } from '../../domain';
import { DiscoverScreen } from '../DiscoverScreen';
import type { InstallHandler } from '../DiscoverScreen';
import { I18nProvider } from '../i18n';
import type { Locale } from '../i18n';
import { LauncherContext } from '../LauncherContext';
import { createFakeController } from './fakeController';

const entry: CatalogEntry = {
  id: 'rebalanced-ai',
  title: 'Rebalanced AI',
  author: 'Ana Rider',
  game: 'MotoGP 25',
  category: 'Physics',
  description: 'Smarter AI.',
  version: '2.3.1',
  updatedAt: '2026-11-10',
  pageUrl: 'not a url',
  thumbnailUrl: null,
  download: { url: 'https://github.com/x/y.zip', size: 10, sha256: 'a'.repeat(64) },
};

const catalogOf = (entries: CatalogEntry[]): Catalog => ({ entries, fetchedAt: 0, stale: false, skipped: 0 });

function setup(options: { locale?: Locale; onInstall?: InstallHandler; refresh?: () => Promise<Catalog> } = {}) {
  const gateway: CatalogGateway = {
    getCatalog: () => Promise.resolve(catalogOf([entry])),
    refreshCatalog: options.refresh ?? (() => Promise.resolve(catalogOf([entry]))),
    openCatalogPage: () => Promise.resolve(),
  };
  const catalog = createCatalogController({ gateway });
  const launcher = createFakeController();
  const onGoToLibrary = vi.fn();
  render(
    <I18nProvider locale={options.locale ?? 'en'}>
      <LauncherContext.Provider value={{ controller: launcher, state: launcher.getState() }}>
        <DiscoverScreen catalog={catalog} onGoToLibrary={onGoToLibrary} onInstall={options.onInstall} />
      </LauncherContext.Provider>
    </I18nProvider>,
  );
  return { catalog, user: userEvent.setup() };
}

afterEach(cleanup);

describe('DiscoverScreen', () => {
  it('shows Install only when the entry has a download and a handler is wired (#74 slot)', async () => {
    const onInstall = vi.fn();
    const { user } = setup({ onInstall });
    await user.click(await screen.findByRole('button', { name: 'Rebalanced AI' }));
    const dialog = screen.getByRole('dialog');
    // An unparsable page URL hides the host line but keeps Open page.
    expect(within(dialog).queryByText(/default browser/)).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Install' }));
    expect(onInstall).toHaveBeenCalledWith(entry);
  });

  it('keeps focus inside the sheet and closes from the scrim', async () => {
    const { user } = setup({ onInstall: vi.fn() });
    await user.click(await screen.findByRole('button', { name: 'Rebalanced AI' }));
    const dialog = screen.getByRole('dialog');
    const close = within(dialog).getByRole('button', { name: 'Close' });
    const openPage = within(dialog).getByRole('button', { name: 'Open page' });
    expect(close).toHaveFocus();
    await user.tab({ shift: true });
    expect(openPage).toHaveFocus();
    await user.tab();
    expect(close).toHaveFocus();
    await user.click(document.querySelector('.sheet-scrim') as HTMLElement);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('keeps the catalog shown when a refresh fails, with a dismissible error', async () => {
    let fail: (error: CommandError) => void = () => undefined;
    const { user } = setup({
      refresh: () =>
        new Promise<Catalog>((_, reject) => {
          fail = reject;
        }),
    });
    await screen.findByRole('button', { name: 'Rebalanced AI' });
    await user.click(screen.getByRole('button', { name: 'Refresh catalog' }));
    expect(screen.getByRole('button', { name: 'Refreshing…' })).toBeDisabled();
    await act(async () => fail({ code: 'catalog_unavailable', message: 'timeout' }));
    expect(screen.getByRole('alert')).toHaveTextContent('timeout');
    expect(screen.getByRole('button', { name: 'Rebalanced AI' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('is translated to pt-BR', async () => {
    setup({ locale: 'pt-BR' });
    expect(await screen.findByRole('heading', { level: 2, name: 'Descobrir' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Atualizar catálogo' })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: 'Ordenar por' })).toBeInTheDocument();
  });
});
