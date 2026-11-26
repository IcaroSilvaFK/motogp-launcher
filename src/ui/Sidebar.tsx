import { useId } from 'react';
import { AdSlot } from './AdSlot';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { SupportLink } from './SupportLink';

export type ScreenId = 'library' | 'discover' | 'profiles' | 'settings';

/** Every screen, in sidebar order. */
export const ALL_SCREENS: readonly ScreenId[] = ['library', 'discover', 'profiles', 'settings'];

/**
 * Left sidebar: the main navigation (icon + label, `aria-current="page"` on the current screen)
 * and a footer pinned at the bottom: the "Support the project" entry (#94) and the collapse toggle. The
 * expanded footer also holds the sidebar sponsored card (#104). The custom title bar already shows the mark and the
 * app name, so the sidebar has no brand row (#71); the document's single h1 (the app name) stays
 * here, visually hidden. Collapsed, only the icons show: the labels stay in the DOM visually
 * hidden, and each item gets its label as a tooltip.
 */
export function Sidebar({
  screens = ALL_SCREENS,
  current,
  onSelect,
  collapsed,
  onToggle,
}: {
  /** The screens to list (Discover only when a catalog is wired); defaults to all of them. */
  screens?: readonly ScreenId[];
  current: ScreenId;
  onSelect: (screen: ScreenId) => void;
  collapsed: boolean;
  onToggle: () => void;
}) {
  const { t } = useI18n();
  const navId = useId();
  const hidden = collapsed ? 'visually-hidden' : undefined;
  const toggleLabel = collapsed ? t.nav.expand : t.nav.collapse;

  return (
    <aside className="sidebar" data-collapsed={String(collapsed)}>
      <h1 className="visually-hidden">{t.appName}</h1>
      <nav id={navId} aria-label={t.nav.label} className="side-nav">
        {screens.map((screen) => (
          <button
            key={screen}
            type="button"
            className="nav-item"
            aria-current={current === screen ? 'page' : undefined}
            title={collapsed ? t.nav[screen] : undefined}
            onClick={() => onSelect(screen)}
          >
            <Icon name={screen} className="nav-icon" />
            <span className={hidden ?? 'nav-label'}>{t.nav[screen]}</span>
          </button>
        ))}
      </nav>
      {/* Sponsored card (#104, #136): only while expanded, in its own region that takes just the
          free height between the navigation and the footer, pinned to its bottom. When that height
          is below the card's budget the card is hidden whole (CSS container query), never cut off,
          and the support entry and the toggle below never move. */}
      <div className="sidebar-ad">{!collapsed && <AdSlot slot="sidebar" />}</div>
      <div className="sidebar-footer">
        <SupportLink placement="sidebar" collapsed={collapsed} />
        <button
          type="button"
          className="sidebar-toggle"
          aria-expanded={!collapsed}
          aria-controls={navId}
          title={toggleLabel}
          onClick={onToggle}
        >
          <Icon name={collapsed ? 'chevron-right' : 'chevron-left'} className="nav-icon" />
          <span className={hidden ?? 'nav-label'}>{toggleLabel}</span>
        </button>
      </div>
    </aside>
  );
}
