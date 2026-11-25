import { useEffect, useId, useRef, useState } from 'react';
import type { KeyboardEvent, ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { categoryCounts, inLibrary, pageHost, selectEntries, sortEntries, useCatalogState } from '../application';
import type { CatalogController, CatalogSort } from '../application';
import type { CatalogEntry, CommandError } from '../domain';
import { BannerIcon } from './BannerIcon';
import { Icon } from './Icon';
import type { IconName } from './Icon';
import { formatDay, formatInstantDate, useI18n } from './i18n';
import { useLauncher } from './LauncherContext';
import { stagger } from './motion';

type View = 'grid' | 'list';

/**
 * Install slot for #74. The Install button renders only when the entry has a `download` block AND
 * this handler is provided; App does not pass one yet, so Install stays hidden.
 */
export type InstallHandler = (entry: CatalogEntry) => void;

const SKELETON_CARDS = [0, 1, 2, 3, 4, 5];

/** Inline error row (refresh or Open page failure) with a Dismiss action. */
function InlineError({ error, onDismiss }: { error: CommandError; onDismiss: () => void }) {
  const { t } = useI18n();
  return (
    <div role="alert" className="notice notice-error">
      <BannerIcon variant="error" />
      <p>
        {t.errors[error.code]}
        {error.message !== '' && <span className="notice-detail"> {error.message}</span>}
      </p>
      <button type="button" className="btn btn-ghost" onClick={onDismiss}>
        {t.dismiss}
      </button>
    </div>
  );
}

/** Two-option segmented button (Material 3), icon only, each option named for screen readers. */
function ViewToggle({ view, onChange }: { view: View; onChange: (view: View) => void }) {
  const { t } = useI18n();
  const options: { value: View; icon: IconName; label: string }[] = [
    { value: 'grid', icon: 'grid', label: t.discover.gridView },
    { value: 'list', icon: 'list', label: t.discover.listView },
  ];
  return (
    <div role="group" aria-label={t.discover.viewLabel} className="segmented">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className="segmented-option"
          aria-pressed={view === option.value}
          aria-label={option.label}
          title={option.label}
          onClick={() => onChange(option.value)}
        >
          <Icon name={option.icon} />
        </button>
      ))}
    </div>
  );
}

/** Thumbnails are left out on purpose (no remote images, CSP unchanged): a neutral category cover. */
function CategoryCover({ category }: { category: string }) {
  return (
    <span className="catalog-cover" aria-hidden="true">
      <Icon name="library" className="catalog-cover-icon" />
      <span className="catalog-cover-label">{category}</span>
    </span>
  );
}

/** Modal side sheet with the entry details. Esc, the scrim and Close dismiss it; focus returns to the opener. */
function EntryDetail({
  entry,
  owned,
  pageError,
  onOpenPage,
  onDismissPageError,
  onClose,
  onInstall,
}: {
  entry: CatalogEntry;
  owned: boolean;
  pageError: CommandError | null;
  onOpenPage: () => void;
  onDismissPageError: () => void;
  onClose: () => void;
  onInstall?: InstallHandler;
}) {
  const { t, locale } = useI18n();
  const titleId = useId();
  const sheetRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const host = pageHost(entry);

  useEffect(() => {
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    closeRef.current?.focus();
    return () => opener?.focus();
  }, []);

  // Esc closes; Tab cycles inside the sheet, so focus never reaches the page behind the scrim.
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== 'Tab' || sheetRef.current === null) return;
    const focusable = [...sheetRef.current.querySelectorAll<HTMLElement>('button:not([disabled])')];
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  };

  const facts: { label: string; value: ReactNode }[] = [
    { label: t.discover.detail.version, value: <span className="num">{entry.version}</span> },
    { label: t.discover.detail.author, value: entry.author },
    { label: t.discover.detail.updated, value: formatDay(entry.updatedAt, locale) },
    { label: t.discover.detail.category, value: entry.category },
    { label: t.discover.detail.game, value: entry.game },
  ];
  if (host !== null) facts.push({ label: t.discover.detail.page, value: host });

  return createPortal(
    <div className="sheet-layer">
      <div className="sheet-scrim" aria-hidden="true" onClick={onClose} />
      <div
        ref={sheetRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="sheet"
        onKeyDown={onKeyDown}
      >
        <div className="sheet-head">
          <div className="sheet-heading">
            <p className="sheet-overline">{entry.category}</p>
            <h2 id={titleId}>{entry.title}</h2>
            <p className="page-subtitle">{t.discover.byAuthor(entry.author)}</p>
          </div>
          <button
            ref={closeRef}
            type="button"
            className="btn btn-ghost btn-icon"
            aria-label={t.discover.detail.close}
            title={t.discover.detail.close}
            onClick={onClose}
          >
            <Icon name="close" />
          </button>
        </div>
        <div className="sheet-body">
          {owned && (
            <span className="badge badge-on" title={t.discover.inLibraryDetail}>
              <Icon name="check" />
              {t.discover.inLibrary}
            </span>
          )}
          <section aria-label={t.discover.detail.description}>
            <p className="sheet-description">{entry.description}</p>
          </section>
          <dl className="details sheet-facts">
            {facts.map((fact) => (
              <div key={fact.label}>
                <dt>{fact.label}</dt>
                <dd>{fact.value}</dd>
              </div>
            ))}
          </dl>
          {pageError !== null && <InlineError error={pageError} onDismiss={onDismissPageError} />}
        </div>
        <div className="sheet-actions">
          {host !== null && <p className="hint">{t.discover.detail.openPageHint(host)}</p>}
          <div className="sheet-buttons">
            {entry.download !== null && onInstall && (
              <button type="button" className="btn" onClick={() => onInstall(entry)}>
                <Icon name="import" />
                {t.discover.detail.install}
              </button>
            )}
            <button type="button" className="btn btn-accent-outline" onClick={onOpenPage}>
              <Icon name="external" />
              {t.discover.detail.openPage}
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}

function LoadingCatalog() {
  const { t } = useI18n();
  return (
    <>
      <p role="status" className="visually-hidden">
        {t.discover.loading}
      </p>
      <div className="category-tiles" aria-hidden="true">
        {[0, 1, 2].map((tile) => (
          <span key={tile} className="skeleton skeleton-category" />
        ))}
      </div>
      <div className="catalog-results" data-view="grid" aria-hidden="true">
        {SKELETON_CARDS.map((card) => (
          <div key={card} className="catalog-card catalog-card-skeleton">
            <span className="skeleton skeleton-cover" />
            <span className="skeleton skeleton-line" />
            <span className="skeleton skeleton-chip" />
          </div>
        ))}
      </div>
    </>
  );
}

/**
 * Discover tab (#73): browse the curated online catalog. Loads on the first visit; the page of an
 * entry opens by id only (the backend owns the URL and its allow-list). `onInstall` is the #74 slot.
 */
export function DiscoverScreen({
  catalog,
  onGoToLibrary,
  onInstall,
}: {
  catalog: CatalogController;
  onGoToLibrary: () => void;
  onInstall?: InstallHandler;
}) {
  const { t, locale } = useI18n();
  const { state: launcher } = useLauncher();
  const state = useCatalogState(catalog);
  const [category, setCategory] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [sort, setSort] = useState<CatalogSort>('updated');
  const [view, setView] = useState<View>('grid');
  const [openId, setOpenId] = useState<string | null>(null);

  useEffect(() => {
    void catalog.load();
  }, [catalog]);

  const loading = state.status === 'idle' || state.status === 'loading';
  const entries = state.catalog?.entries ?? [];
  const busy = loading || state.refreshing;

  const head = (actions: boolean) => (
    <div className="page-head">
      <div className="page-heading">
        <h2 id="discover-title">{t.discover.title}</h2>
        {state.catalog !== null && (
          <p className="page-subtitle">
            {t.discover.summary(entries.length, formatInstantDate(state.catalog.fetchedAt, locale))}
          </p>
        )}
      </div>
      {actions && (
        <div className="page-actions">
          <button type="button" className="btn" disabled={busy} onClick={() => void catalog.refresh()}>
            <Icon name="refresh" />
            {state.refreshing ? t.discover.refreshing : t.discover.refresh}
          </button>
        </div>
      )}
    </div>
  );

  const page = (actions: boolean, body: ReactNode) => (
    <section aria-labelledby="discover-title" aria-busy={busy || undefined} className="page">
      {head(actions)}
      {body}
    </section>
  );

  if (loading) return page(false, <LoadingCatalog />);

  if (state.status === 'error' && state.error !== null) {
    return page(
      false,
      <div role="alert" className="empty-state">
        <span className="empty-icon" aria-hidden="true">
          <Icon name="discover" />
        </span>
        <p className="empty-title">{t.errors[state.error.code]}</p>
        {state.error.message !== '' && <p className="hint">{state.error.message}</p>}
        <button type="button" className="btn btn-accent-outline" onClick={() => void catalog.refresh()}>
          <Icon name="refresh" />
          {t.discover.retry}
        </button>
      </div>,
    );
  }

  const banners = (
    <>
      {state.catalog?.stale === true && (
        <div role="status" className="notice notice-warning">
          <BannerIcon variant="warning" />
          <p>{t.discover.stale(formatInstantDate(state.catalog.fetchedAt, locale))}</p>
        </div>
      )}
      {state.error !== null && <InlineError error={state.error} onDismiss={() => catalog.dismissError()} />}
    </>
  );

  if (entries.length === 0) {
    return page(
      false,
      <>
        {banners}
        <div className="empty-state discover-empty">
          <span className="empty-icon" aria-hidden="true">
            <Icon name="discover" />
          </span>
          <p className="empty-title">{t.discover.emptyTitle}</p>
          <p>{t.discover.emptyCurated}</p>
          <p>{t.discover.emptyHowTo}</p>
          <p className="hint">{t.discover.emptyMeanwhile}</p>
          <div className="empty-actions">
            <button type="button" className="btn btn-accent-outline" onClick={onGoToLibrary}>
              <Icon name="library" />
              {t.discover.goToLibrary}
            </button>
            <button type="button" className="btn" disabled={busy} onClick={() => void catalog.refresh()}>
              <Icon name="refresh" />
              {state.refreshing ? t.discover.refreshing : t.discover.refresh}
            </button>
          </div>
        </div>
      </>,
    );
  }

  const tiles: { category: string | null; label: string; count: number }[] = [
    { category: null, label: t.discover.all, count: entries.length },
    ...categoryCounts(entries).map((item) => ({ category: item.category, label: item.category, count: item.count })),
  ];
  const visible = sortEntries(selectEntries(entries, { category, query }), sort);
  const open = openId === null ? undefined : entries.find((entry) => entry.id === openId);

  const clearFilters = () => {
    setCategory(null);
    setQuery('');
  };

  return page(
    true,
    <>
      {banners}
      <div role="group" aria-label={t.discover.categories} className="category-tiles">
        {tiles.map((tile) => (
          <button
            key={tile.category ?? ''}
            type="button"
            className="category-tile"
            aria-pressed={category === tile.category}
            onClick={() => setCategory(tile.category)}
          >
            <span className="category-tile-label">{tile.label}</span>{' '}
            <span className="facet-count num">{tile.count}</span>
          </button>
        ))}
      </div>

      <div className="catalog-toolbar">
        <div className="search catalog-search">
          <label htmlFor="discover-search" className="visually-hidden">
            {t.discover.search}
          </label>
          <Icon name="search" className="search-icon" />
          <input
            id="discover-search"
            type="search"
            className="input"
            placeholder={t.discover.searchPlaceholder}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
        <label className="field">
          <span className="field-label">{t.discover.sortLabel}</span>
          <select value={sort} onChange={(event) => setSort(event.target.value as CatalogSort)}>
            <option value="updated">{t.discover.sortUpdated}</option>
            <option value="name">{t.discover.sortName}</option>
          </select>
        </label>
        <ViewToggle view={view} onChange={setView} />
        <p className="hint catalog-count" aria-live="polite">
          {t.discover.resultCount(visible.length)}
        </p>
      </div>

      {visible.length === 0 ? (
        <div className="empty-state empty-state-compact">
          <p>{t.discover.noResults}</p>
          <button type="button" className="btn btn-sm" onClick={clearFilters}>
            {t.discover.clearFilters}
          </button>
        </div>
      ) : (
        <ul className="catalog-results" data-view={view} aria-label={t.discover.results}>
          {visible.map((entry, index) => {
            const owned = inLibrary(entry, launcher.mods);
            const key = `discover-${index}`;
            // The whole card is one button named by the title; the rest of the card describes it.
            return (
              <li key={entry.id} className="stagger-item" style={stagger(index)}>
                <button
                  type="button"
                  className="catalog-card"
                  aria-haspopup="dialog"
                  aria-labelledby={`${key}-title`}
                  aria-describedby={`${key}-meta`}
                  onClick={() => setOpenId(entry.id)}
                >
                  <CategoryCover category={entry.category} />
                  <span className="catalog-card-body">
                    <span id={`${key}-title`} className="catalog-card-title">
                      {entry.title}
                    </span>
                    <span id={`${key}-meta`} className="catalog-card-meta">
                      <span className="catalog-card-author">{t.discover.byAuthor(entry.author)}</span>
                      <span className="mod-meta">
                        <span>{entry.category}</span>
                        <span className="dot" aria-hidden="true" />
                        <span className="num">{t.discover.version(entry.version)}</span>
                        <span className="dot" aria-hidden="true" />
                        <span>{t.discover.updated(formatDay(entry.updatedAt, locale))}</span>
                      </span>
                      {owned && (
                        <span className="badge badge-on catalog-card-badge" title={t.discover.inLibraryDetail}>
                          <Icon name="check" />
                          {t.discover.inLibrary}
                        </span>
                      )}
                    </span>
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}

      {open !== undefined && (
        <EntryDetail
          entry={open}
          owned={inLibrary(open, launcher.mods)}
          pageError={state.pageError}
          onOpenPage={() => void catalog.openPage(open.id)}
          onDismissPageError={() => catalog.dismissPageError()}
          onClose={() => {
            catalog.dismissPageError();
            setOpenId(null);
          }}
          onInstall={onInstall}
        />
      )}
    </>,
  );
}
