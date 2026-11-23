import { useEffect, useState } from 'react';
import { Icon } from './Icon';
import { formatSize, useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

type Facet = 'all' | 'in' | 'out';

const FACETS: readonly Facet[] = ['all', 'in', 'out'];

export function LibraryScreen() {
  const { t, locale } = useI18n();
  const { controller, state } = useLauncher();
  const [search, setSearch] = useState('');
  const [facet, setFacet] = useState<Facet>('all');

  // Drop listener: released exactly once, even if the subscription resolves after unmount.
  useEffect(() => {
    let released = false;
    let release: (() => void) | null = null;
    void controller.subscribeDrop().then((unsubscribe) => {
      if (released) unsubscribe();
      else release = unsubscribe;
    });
    return () => {
      released = true;
      release?.();
      release = null;
    };
  }, [controller]);

  const activeProfile = state.profiles.find((profile) => profile.id === state.config.active_profile);
  const activeModIds = new Set(activeProfile?.mod_ids ?? []);
  const inProfile = state.mods.filter((mod) => activeModIds.has(mod.id)).length;
  const counts: Record<Facet, number> = {
    all: state.mods.length,
    in: inProfile,
    out: state.mods.length - inProfile,
  };
  const facetLabel: Record<Facet, string> = {
    all: t.library.filterAll,
    in: t.library.inActiveProfile,
    out: t.library.notInActiveProfile,
  };

  // Client-side filters: name search plus the "in the active profile" facet.
  const query = search.trim().toLocaleLowerCase();
  const visible = state.mods.filter(
    (mod) =>
      mod.name.toLocaleLowerCase().includes(query) &&
      (facet === 'all' || (facet === 'in') === activeModIds.has(mod.id)),
  );

  const clearFilters = () => {
    setSearch('');
    setFacet('all');
  };

  const head = (actions: boolean) => (
    <div className="page-head">
      <div className="page-heading">
        <h2 id="library-title">{t.library.title}</h2>
        <p className="page-subtitle">{t.library.summary(state.mods.length, inProfile, activeProfile?.name ?? null)}</p>
      </div>
      {actions && (
        <div className="page-actions">
          <button type="button" className="btn" onClick={() => void controller.importFromDialog()}>
            <Icon name="import" />
            {t.library.import}
          </button>
        </div>
      )}
    </div>
  );

  if (state.mods.length === 0) {
    return (
      <section aria-labelledby="library-title" className="page">
        {head(false)}
        <div className="empty-state">
          <span className="empty-icon" aria-hidden="true">
            <Icon name="library" />
          </span>
          <p className="empty-title">{t.library.empty}</p>
          <p className="hint">{t.library.dropHint}</p>
          <button type="button" className="btn btn-accent-outline" onClick={() => void controller.importFromDialog()}>
            <Icon name="import" />
            {t.library.import}
          </button>
        </div>
      </section>
    );
  }

  return (
    <section aria-labelledby="library-title" className="page">
      {head(true)}
      <div className="library-layout">
        <aside className="filter-rail">
          <div className="search">
            <label htmlFor="library-search" className="visually-hidden">
              {t.library.search}
            </label>
            <Icon name="search" className="search-icon" />
            <input
              id="library-search"
              type="search"
              className="input"
              placeholder={t.library.searchPlaceholder}
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </div>
          <div role="group" aria-labelledby="library-filter-title" className="facet-group">
            <p id="library-filter-title" className="rail-title">
              <Icon name="filter" />
              {t.library.filterLabel}
            </p>
            {FACETS.map((option) => (
              <button
                key={option}
                type="button"
                className="facet"
                aria-pressed={facet === option}
                onClick={() => setFacet(option)}
              >
                <span className="facet-label">{facetLabel[option]}</span>{' '}
                <span className="facet-count num">{counts[option]}</span>
              </button>
            ))}
          </div>
          <p className="hint drop-hint">{t.library.dropHint}</p>
        </aside>

        <div className="library-main">
          {visible.length === 0 ? (
            <div className="empty-state empty-state-compact">
              <p>{t.library.noResults}</p>
              <button type="button" className="btn btn-sm" onClick={clearFilters}>
                {t.library.clearFilters}
              </button>
            </div>
          ) : (
            <ul className="mod-list" aria-labelledby="library-title">
              {visible.map((mod) => {
                const enabled = activeModIds.has(mod.id);
                return (
                  <li key={mod.id} className="mod-row">
                    <span className="mod-tile" aria-hidden="true">
                      {mod.name.trim().charAt(0).toLocaleUpperCase()}
                    </span>
                    <div className="mod-main">
                      <span className="mod-name" title={mod.name}>
                        {mod.name}
                      </span>
                      <span className="mod-meta">
                        <span className="num">{formatSize(mod.size_bytes, locale)}</span>
                        <span className="dot" aria-hidden="true" />
                        <span className="num">{t.library.files(mod.files.length)}</span>
                      </span>
                    </div>
                    <span className={enabled ? 'badge badge-on' : 'badge badge-off'}>
                      {enabled ? t.library.inActiveProfile : t.library.notInActiveProfile}
                    </span>
                    <button
                      type="button"
                      className="btn btn-danger btn-sm"
                      aria-label={t.library.removeNamed(mod.name)}
                      onClick={() => void controller.removeMod(mod.id)}
                    >
                      {t.library.remove}
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </div>
    </section>
  );
}
