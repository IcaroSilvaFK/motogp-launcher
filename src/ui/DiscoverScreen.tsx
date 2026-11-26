import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { MouseEvent, ReactNode } from 'react';
import {
  categoryCounts,
  inLibrary,
  loadDiscoverView,
  saveDiscoverView,
  selectEntries,
  sortEntries,
  useCatalogState,
  useModStats,
} from '../application';
import type { CatalogController, CatalogSort, DiscoverView, DiscoverViewStore } from '../application';
import { AdSlot } from './AdSlot';
import { BannerIcon } from './BannerIcon';
import { CategoryCover, InlineError } from './CatalogParts';
import { useCommunity } from './CommunityContext';
import { Icon } from './Icon';
import type { IconName } from './Icon';
import { formatDay, formatInstantDate, useI18n } from './i18n';
import { useLauncher } from './LauncherContext';
import { ModPage } from './ModPage';
import { stagger } from './motion';
import { StarCount } from './StarButton';

type View = DiscoverView;

/** Where the list was when a mod page opened, so going back lands on the same spot (#115). */
type ReturnPoint = {
  entryId: string;
  scroller: Element | null;
  scrollTop: number;
  /** False when going back changes the filters (breadcrumb category, author): the old position is meaningless. */
  restoreScroll: boolean;
};

const SKELETON_CARDS = [0, 1, 2, 3, 4, 5];

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
 * entry opens by id only (the backend owns the URL and its allow-list). Entries with a download show the install control (#74).
 */
export function DiscoverScreen({
  catalog,
  onGoToLibrary,
  viewStore,
}: {
  catalog: CatalogController;
  onGoToLibrary: () => void;
  /** Remembers the grid/list choice across restarts (#89). */
  viewStore?: DiscoverViewStore;
}) {
  const { t, locale } = useI18n();
  const { state: launcher } = useLauncher();
  const state = useCatalogState(catalog);
  const [category, setCategory] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [sort, setSort] = useState<CatalogSort>('updated');
  // Read once on mount; without a store the choice lives in memory and starts in the grid.
  const [view, setView] = useState<View>(() => (viewStore ? loadDiscoverView(viewStore) : 'grid'));
  // The save stays out of the state updater, which StrictMode may run twice.
  const changeView = (next: View) => {
    setView(next);
    if (viewStore) saveDiscoverView(viewStore, next);
  };
  const [openId, setOpenId] = useState<string | null>(null);
  const returnPoint = useRef<ReturnPoint | null>(null);
  // Community stars (#119): one cached load per visit; offline or unwired, no counts are shown.
  const { stars: starsStore } = useCommunity();
  const starStats = useModStats(starsStore);
  const showStars = starStats.stats.size > 0;
  const starsOf = (id: string) => starStats.stats.get(id)?.stars ?? 0;

  useEffect(() => {
    void catalog.load();
  }, [catalog]);

  // Opening a mod page starts it at the top; going back restores the list's scroll position and
  // puts focus back on the card that opened it. Layout effect: before paint, so nothing jumps.
  useLayoutEffect(() => {
    const point = returnPoint.current;
    if (point === null) return;
    if (openId !== null) {
      if (point.scroller !== null) point.scroller.scrollTop = 0;
      return;
    }
    returnPoint.current = null;
    if (point.restoreScroll && point.scroller !== null) point.scroller.scrollTop = point.scrollTop;
    const card = [...document.querySelectorAll<HTMLElement>('[data-entry-id]')].find(
      (element) => element.dataset.entryId === point.entryId,
    );
    card?.focus({ preventScroll: point.restoreScroll });
  }, [openId]);

  const openEntry = (entryId: string, event: MouseEvent<HTMLElement>) => {
    // `scrollingElement` is missing in some environments (jsdom), hence the last fallback.
    const scroller = event.currentTarget.closest('main') ?? document.scrollingElement ?? null;
    returnPoint.current = { entryId, scroller, scrollTop: scroller?.scrollTop ?? 0, restoreScroll: true };
    setOpenId(entryId);
  };

  const closeEntry = (filters?: { category?: string; query?: string }) => {
    catalog.dismissPageError();
    if (filters !== undefined && returnPoint.current !== null) returnPoint.current.restoreScroll = false;
    if (filters?.category !== undefined) setCategory(filters.category);
    if (filters?.query !== undefined) {
      setCategory(null);
      setQuery(filters.query);
    }
    setOpenId(null);
  };

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
        <button type="button" className="btn btn-accent-outline" onClick={() => void catalog.refresh()}>
          <Icon name="refresh" />
          {t.discover.retry}
        </button>
      </div>,
    );
  }

  const banners = (
    <>
      {state.catalog?.source === 'bundled' && (
        <div role="status" className="notice notice-info">
          <BannerIcon variant="info" />
          <p>{t.discover.bundled}</p>
        </div>
      )}
      {state.catalog?.source === 'cache' && state.catalog.stale && (
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

  const open = openId === null ? undefined : entries.find((entry) => entry.id === openId);
  if (open !== undefined) {
    return (
      <ModPage
        key={open.id}
        entry={open}
        authorCount={entries.filter((entry) => entry.author === open.author).length}
        owned={inLibrary(open, launcher.mods)}
        pageError={state.pageError}
        onOpenPage={() => void catalog.openPage(open.id)}
        onDismissPageError={() => catalog.dismissPageError()}
        onBack={() => closeEntry()}
        onCategory={() => closeEntry({ category: open.category })}
        onAuthor={() => closeEntry({ query: open.author })}
      />
    );
  }

  const tiles: { category: string | null; label: string; count: number }[] = [
    { category: null, label: t.discover.all, count: entries.length },
    ...categoryCounts(entries).map((item) => ({ category: item.category, label: item.category, count: item.count })),
  ];
  const visible = sortEntries(selectEntries(entries, { category, query }), sort, starsOf);

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
            {starsStore && <option value="stars">{t.stars.sort}</option>}
          </select>
        </label>
        <ViewToggle view={view} onChange={changeView} />
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
                  data-entry-id={entry.id}
                  aria-labelledby={`${key}-title`}
                  aria-describedby={`${key}-meta`}
                  onClick={(event) => openEntry(entry.id, event)}
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
                        {showStars && <span className="dot" aria-hidden="true" />}
                        {showStars && <StarCount stars={starsOf(entry.id)} />}
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

      {/* Sponsored card (#104): after the results, outside the list, the tiles and the counts, so it
          is never counted or matched by search, and inserting it after load moves nothing above it. */}
      <AdSlot slot="discover" />
    </>,
  );
}
