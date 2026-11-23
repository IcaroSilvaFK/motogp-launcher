import { Icon } from './Icon';
import { useI18n } from './i18n';

export type ScreenId = 'library' | 'profiles' | 'settings';

const SCREENS: readonly ScreenId[] = ['library', 'profiles', 'settings'];

/** Top tab navigation: one icon + label tab per screen, the current one marked with aria-current. */
export function TopNav({ current, onSelect }: { current: ScreenId; onSelect: (screen: ScreenId) => void }) {
  const { t } = useI18n();
  return (
    <nav aria-label={t.nav.label} className="top-nav">
      {SCREENS.map((screen) => (
        <button
          key={screen}
          type="button"
          className="nav-tab"
          aria-current={current === screen ? 'page' : undefined}
          onClick={() => onSelect(screen)}
        >
          <Icon name={screen} className="nav-icon" />
          {t.nav[screen]}
        </button>
      ))}
    </nav>
  );
}
