import { useEffect, useId, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { useMyMods, usePublishMod } from '../application';
import { useCommunity } from './CommunityContext';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { MyModsList } from './MyModsList';
import { PublishModForm } from './PublishModForm';

type Tab = 'publish' | 'mine';
const TABS: Tab[] = ['publish', 'mine'];

/**
 * Publishing inside Discover (#130): back and breadcrumb like the mod page, then two tabs, the
 * publish form and "My mods". Both panels stay mounted (the inactive one hidden) so switching tabs
 * keeps a half-filled form. Esc and Back return to Discover, except while an upload runs.
 */
export function PublishScreen({
  creator,
  categories,
  games,
  onBack,
}: {
  /** The signed-in creator's display name. */
  creator: string;
  /** Suggestions for the category and game fields (from the catalog). */
  categories: string[];
  games: string[];
  onBack: () => void;
}) {
  const { t } = useI18n();
  const { publisher, publishPicker } = useCommunity();
  const publish = usePublishMod(publisher);
  const myMods = useMyMods(publisher);
  const [tab, setTab] = useState<Tab>('publish');
  const titleId = useId();
  const tabsId = useId();
  const backRef = useRef<HTMLButtonElement>(null);
  const tabRefs = useRef<Record<Tab, HTMLButtonElement | null>>({ publish: null, mine: null });
  const page = t.publish;

  // The page replaces the list, so focus moves to Back, like the mod page.
  useEffect(() => {
    backRef.current?.focus();
  }, []);

  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === 'Escape' && !event.defaultPrevented && !publish.busy) {
      event.preventDefault();
      onBack();
    }
  };

  const selectTab = (next: Tab, focus = false) => {
    setTab(next);
    if (focus) tabRefs.current[next]?.focus();
  };

  // Tabs (WAI-ARIA): arrows move and select, Home/End jump; only the selected tab is in the Tab order.
  const onTabKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    const index = TABS.indexOf(tab);
    let next: Tab | undefined;
    if (event.key === 'ArrowRight') next = TABS[(index + 1) % TABS.length];
    else if (event.key === 'ArrowLeft') next = TABS[(index - 1 + TABS.length) % TABS.length];
    else if (event.key === 'Home') next = TABS[0];
    else if (event.key === 'End') next = TABS[TABS.length - 1];
    if (next === undefined) return;
    event.preventDefault();
    selectTab(next, true);
  };

  const modCount = myMods.status === 'ready' ? myMods.mods.length : null;
  const labels: Record<Tab, string> = { publish: page.tabPublish, mine: page.tabMyMods };

  return (
    <section aria-labelledby={titleId} className="page publish-page" onKeyDown={onKeyDown}>
      <div className="mod-nav">
        <button
          ref={backRef}
          type="button"
          className="btn btn-ghost btn-sm mod-back"
          aria-label={page.backTo}
          disabled={publish.busy}
          onClick={onBack}
        >
          <Icon name="chevron-left" />
          {page.back}
        </button>
        <nav aria-label={t.discover.modPage.breadcrumb} className="breadcrumb">
          <ol>
            <li>
              <button type="button" className="breadcrumb-link" disabled={publish.busy} onClick={onBack}>
                {t.discover.title}
              </button>
            </li>
            <li>
              <span className="breadcrumb-current" aria-current="page">
                {page.open}
              </span>
            </li>
          </ol>
        </nav>
      </div>

      <div className="page-head">
        <div className="page-heading">
          <h2 id={titleId}>{page.open}</h2>
          <p className="page-subtitle">{page.subtitle(creator)}</p>
        </div>
      </div>

      <div role="tablist" aria-label={page.tabsLabel} className="tabs">
        {TABS.map((value) => (
          <button
            key={value}
            ref={(element) => {
              tabRefs.current[value] = element;
            }}
            type="button"
            role="tab"
            id={`${tabsId}-${value}-tab`}
            aria-selected={tab === value}
            aria-controls={`${tabsId}-${value}-panel`}
            tabIndex={tab === value ? 0 : -1}
            className="tab"
            onClick={() => selectTab(value)}
            onKeyDown={onTabKeyDown}
          >
            {labels[value]}
            {value === 'mine' && modCount !== null && modCount > 0 && <span className="tab-count num">{modCount}</span>}
          </button>
        ))}
      </div>

      <div
        role="tabpanel"
        id={`${tabsId}-publish-panel`}
        aria-labelledby={`${tabsId}-publish-tab`}
        className="tab-panel"
        hidden={tab !== 'publish'}
      >
        <PublishModForm
          publish={publish}
          picker={publishPicker}
          categories={categories}
          games={games}
          onPublished={myMods.refresh}
          onViewMyMods={() => selectTab('mine', true)}
        />
      </div>
      <div
        role="tabpanel"
        id={`${tabsId}-mine-panel`}
        aria-labelledby={`${tabsId}-mine-tab`}
        className="tab-panel"
        hidden={tab !== 'mine'}
      >
        <MyModsList myMods={myMods} onPublish={() => selectTab('publish', true)} />
      </div>
    </section>
  );
}
