import { useEffect, useId, useRef, useState } from 'react';
import type { KeyboardEvent, ReactNode } from 'react';
import { pageHost } from '../application';
import type { CatalogEntry, CommandError } from '../domain';
import { AdSlot } from './AdSlot';
import { CategoryCover, InlineError } from './CatalogParts';
import { Icon } from './Icon';
import { formatDay, formatSize, useI18n } from './i18n';
import { CatalogInstallButton } from './InstallButton';
import { ModComments } from './ModComments';
import { SafeMarkdown } from './SafeMarkdown';
import { ScreenshotGallery } from './ScreenshotGallery';
import { StarButton } from './StarButton';

/** Header cover: the entry thumbnail, or the category cover when there is none or it fails. */
function ModCover({ entry }: { entry: CatalogEntry }) {
  const [broken, setBroken] = useState(false);
  if (entry.thumbnailUrl === null || broken) {
    return <CategoryCover category={entry.category} className="catalog-cover mod-cover" />;
  }
  return (
    <img
      className="mod-cover mod-cover-image"
      src={entry.thumbnailUrl}
      alt=""
      decoding="async"
      referrerPolicy="no-referrer"
      onError={() => setBroken(true)}
    />
  );
}

/**
 * Full mod page inside Discover (#115): back and breadcrumb, a header with the one main action,
 * a stats strip, the screenshot gallery, the description in the safe Markdown subset, the details
 * list, and a right column with the author card and the sponsored slot. Esc goes back.
 */
export function ModPage({
  entry,
  authorCount,
  owned,
  pageError,
  onOpenPage,
  onDismissPageError,
  onBack,
  onCategory,
  onAuthor,
}: {
  entry: CatalogEntry;
  /** How many catalog entries share this author, this one included. */
  authorCount: number;
  owned: boolean;
  pageError: CommandError | null;
  onOpenPage: () => void;
  onDismissPageError: () => void;
  onBack: () => void;
  onCategory: () => void;
  onAuthor: () => void;
}) {
  const { t, locale } = useI18n();
  const titleId = useId();
  const authorId = useId();
  const detailsId = useId();
  const backRef = useRef<HTMLButtonElement>(null);
  const host = pageHost(entry);
  const page = t.discover.modPage;

  // The page replaces the list, so focus moves to Back (the breadcrumb follows it).
  useEffect(() => {
    backRef.current?.focus();
  }, []);

  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === 'Escape' && !event.defaultPrevented) {
      event.preventDefault();
      onBack();
    }
  };

  const stats: { label: string; value: ReactNode }[] = [
    { label: t.discover.detail.version, value: <span className="num">{entry.version}</span> },
    { label: t.discover.detail.updated, value: formatDay(entry.updatedAt, locale) },
  ];
  if (entry.download !== null) {
    stats.push({ label: page.size, value: <span className="num">{formatSize(entry.download.size, locale)}</span> });
    // The full hash stays in the text (copyable, selected at once) and in the tooltip; CSS truncates it.
    stats.push({
      label: page.sha256,
      value: (
        <code className="mod-sha" title={entry.download.sha256}>
          {entry.download.sha256}
        </code>
      ),
    });
  }

  const details: { label: string; value: ReactNode }[] = [
    { label: t.discover.detail.author, value: entry.author },
    { label: t.discover.detail.category, value: entry.category },
    { label: t.discover.detail.game, value: entry.game },
  ];
  if (host !== null) details.push({ label: t.discover.detail.page, value: host });

  const hasDownload = entry.download !== null;

  return (
    <section aria-labelledby={titleId} className="page mod-page" onKeyDown={onKeyDown}>
      <div className="mod-nav">
        <button
          ref={backRef}
          type="button"
          className="btn btn-ghost btn-sm mod-back"
          aria-label={page.backTo}
          onClick={onBack}
        >
          <Icon name="chevron-left" />
          {page.back}
        </button>
        <nav aria-label={page.breadcrumb} className="breadcrumb">
          <ol>
            <li>
              <button type="button" className="breadcrumb-link" onClick={onBack}>
                {t.discover.title}
              </button>
            </li>
            <li>
              <button type="button" className="breadcrumb-link" onClick={onCategory}>
                {entry.category}
              </button>
            </li>
            <li>
              <span className="breadcrumb-current" aria-current="page">
                {entry.title}
              </span>
            </li>
          </ol>
        </nav>
      </div>

      <header className="mod-header">
        <ModCover entry={entry} />
        <div className="mod-heading">
          <h2 id={titleId}>{entry.title}</h2>
          <p className="page-subtitle">{t.discover.byAuthor(entry.author)}</p>
          <div className="mod-chips">
            <span className="chip">{entry.category}</span>
            <span className="chip">{entry.game}</span>
            {owned && (
              <span className="badge badge-on" title={t.discover.inLibraryDetail}>
                <Icon name="check" />
                {t.discover.inLibrary}
              </span>
            )}
          </div>
        </div>
        <div className="mod-actions">
          <div className="mod-buttons">
            <StarButton modId={entry.id} />
            {hasDownload && <CatalogInstallButton entryId={entry.id} />}
            {/* One main action: Install when there is a download, otherwise Open page. */}
            <button type="button" className={hasDownload ? 'btn' : 'btn btn-accent-outline'} onClick={onOpenPage}>
              <Icon name="external" />
              {t.discover.detail.openPage}
            </button>
          </div>
          {host !== null && <p className="hint">{t.discover.detail.openPageHint(host)}</p>}
        </div>
      </header>

      {pageError !== null && <InlineError error={pageError} onDismiss={onDismissPageError} />}

      <dl className="mod-stats" aria-label={page.stats}>
        {stats.map((stat) => (
          <div key={stat.label} className="mod-stat">
            <dt>{stat.label}</dt>
            <dd>{stat.value}</dd>
          </div>
        ))}
      </dl>

      <div className="mod-layout">
        <div className="mod-main">
          <ScreenshotGallery screenshots={entry.screenshots} title={entry.title} />
          <section aria-label={t.discover.detail.description} className="mod-description">
            <SafeMarkdown source={entry.longDescription ?? entry.description} />
          </section>
          <section aria-labelledby={detailsId} className="mod-section">
            <h3 id={detailsId} className="group-title">
              {page.details}
            </h3>
            <dl className="details mod-details">
              {details.map((fact) => (
                <div key={fact.label}>
                  <dt>{fact.label}</dt>
                  <dd>{fact.value}</dd>
                </div>
              ))}
            </dl>
          </section>
          <ModComments modId={entry.id} />
        </div>

        <div className="mod-aside">
          <section aria-labelledby={authorId} className="card mod-author">
            <h3 id={authorId} className="group-title">
              {page.author}
            </h3>
            <p className="mod-author-name">{entry.author}</p>
            <p className="hint">{page.authorMods(authorCount)}</p>
            {authorCount > 1 && (
              <button type="button" className="btn btn-sm" onClick={onAuthor}>
                <Icon name="search" />
                {page.moreFromAuthor(entry.author)}
              </button>
            )}
          </section>
          {/* Sponsored card (#104): below the author card, in a column that grows downward, so
              inserting it after load moves nothing above it. */}
          <AdSlot slot="discover" />
        </div>
      </div>
    </section>
  );
}
