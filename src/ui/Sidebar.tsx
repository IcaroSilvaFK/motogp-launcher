import { useId } from 'react';
import { Icon } from './Icon';
import { useI18n } from './i18n';

export type ScreenId = 'library' | 'profiles' | 'settings';

const SCREENS: readonly ScreenId[] = ['library', 'profiles', 'settings'];

/**
 * Left sidebar: brand, the main navigation (icon + label, `aria-current="page"` on the current
 * screen) and a collapse toggle pinned at the bottom. Collapsed, only the icons show: the labels
 * and the h1 stay in the DOM visually hidden, and each item gets its label as a tooltip.
 */
export function Sidebar({
  current,
  onSelect,
  collapsed,
  onToggle,
}: {
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
      <div className="brand">
        <svg className="brand-mark" viewBox="0 0 20 20" aria-hidden="true" focusable="false">
          <rect x="2" y="2" width="16" height="16" rx="4" />
          <path d="M6 13.5l2.6-7 1.4 3.6 1.4-3.6 2.6 7" />
        </svg>
        <h1 className={hidden}>{t.appName}</h1>
      </div>
      <nav id={navId} aria-label={t.nav.label} className="side-nav">
        {SCREENS.map((screen) => (
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
    </aside>
  );
}
