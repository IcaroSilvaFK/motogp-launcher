// DiscoverScreen details outside the Gherkin scenarios: the #74 Install slot, refresh failures,
// the mod page (#115) actions, back, author card and thumbnail fallback, and the pt-BR copy.
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createCatalogController } from '../../application';
import type { CatalogGateway } from '../../application';
import type { Catalog, CatalogEntry, CommandError } from '../../domain';
import { DiscoverScreen } from '../DiscoverScreen';
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
  screenshots: [],
  longDescription: null,
  download: { url: 'https://github.com/x/y.zip', size: 10, sha256: 'a'.repeat(64) },
};

const catalogOf = (entries: CatalogEntry[]): Catalog => ({
  entries,
  fetchedAt: 0,
  stale: false,
  source: 'remote',
  skipped: 0,
});

function setup(
  options: {
    locale?: Locale;
    download?: CatalogEntry['download'];
    thumbnailUrl?: string;
    refresh?: () => Promise<Catalog>;
  } = {},
) {
  const withDownload = options.download === undefined ? entry : { ...entry, download: options.download };
  const shown = { ...withDownload, thumbnailUrl: options.thumbnailUrl ?? null };
  const gateway: CatalogGateway = {
    getCatalog: () => Promise.resolve(catalogOf([shown])),
    refreshCatalog: options.refresh ?? (() => Promise.resolve(catalogOf([entry]))),
    openCatalogPage: () => Promise.resolve(),
  };
  const catalog = createCatalogController({ gateway });
  const launcher = createFakeController();
  const onGoToLibrary = vi.fn();
  render(
    <I18nProvider locale={options.locale ?? 'en'}>
      <LauncherContext.Provider value={{ controller: launcher, state: launcher.getState() }}>
        <DiscoverScreen catalog={catalog} onGoToLibrary={onGoToLibrary} />
      </LauncherContext.Provider>
    </I18nProvider>,
  );
  return { catalog, launcher, user: userEvent.setup() };
}

afterEach(cleanup);

describe('DiscoverScreen', () => {
  it('shows the connected install button for a downloadable entry', async () => {
    const { user, launcher } = setup();
    await user.click(await screen.findByRole('button', { name: 'Rebalanced AI' }));
    const page = screen.getByRole('region', { name: 'Rebalanced AI' });
    // An unparsable page URL hides the host line but keeps Open page.
    expect(within(page).queryByText(/default browser/)).not.toBeInTheDocument();
    // Install is the one main action; Open page steps down to an outlined secondary button.
    expect(within(page).getByRole('button', { name: 'Open page' })).not.toHaveClass('btn-accent-outline');
    await user.click(within(page).getByRole('button', { name: 'Install' }));
    expect(launcher.installCatalogEntry).toHaveBeenCalledWith(entry.id);
  });

  it('shows no install button for an entry without a download', async () => {
    const { user } = setup({ download: null });
    await user.click(await screen.findByRole('button', { name: 'Rebalanced AI' }));
    const page = screen.getByRole('region', { name: 'Rebalanced AI' });
    expect(within(page).queryByRole('button', { name: /install/i })).not.toBeInTheDocument();
    expect(within(page).getByRole('button', { name: 'Open page' })).toHaveClass('btn-accent-outline');
  });

  it('goes back from the breadcrumb and renders the page without a dialog or portal', async () => {
    const { user } = setup();
    await user.click(await screen.findByRole('button', { name: 'Rebalanced AI' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    const crumbs = screen.getByRole('navigation', { name: 'Breadcrumb' });
    await user.click(within(crumbs).getByRole('button', { name: 'Discover' }));
    expect(screen.getByRole('heading', { level: 2, name: 'Discover' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Rebalanced AI' })).toHaveFocus();
  });

  it('shows the author card with a single mod and no "more from" action', async () => {
    const { user } = setup();
    await user.click(await screen.findByRole('button', { name: 'Rebalanced AI' }));
    const author = screen.getByRole('region', { name: 'Author' });
    expect(within(author).getByText('1 mod in the catalog')).toBeInTheDocument();
    expect(within(author).queryByRole('button')).not.toBeInTheDocument();
  });

  it('falls back to the category cover when the thumbnail fails', async () => {
    const { user } = setup({ thumbnailUrl: 'https://img.test/thumb.png' });
    await user.click(await screen.findByRole('button', { name: 'Rebalanced AI' }));
    const page = screen.getByRole('region', { name: 'Rebalanced AI' });
    const thumb = page.querySelector('.mod-cover-image') as HTMLImageElement;
    expect(thumb).toHaveAttribute('src', 'https://img.test/thumb.png');
    expect(thumb).toHaveAttribute('referrerpolicy', 'no-referrer');
    fireEvent.error(thumb);
    expect(page.querySelector('.mod-cover-image')).toBeNull();
    expect(page.querySelector('.catalog-cover.mod-cover')).not.toBeNull();
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
    expect(screen.getByRole('alert')).toHaveTextContent('The mod catalog could not be loaded.');
    expect(screen.getByRole('alert')).not.toHaveTextContent('timeout');
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
