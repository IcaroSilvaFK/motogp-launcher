import { useI18n } from './i18n';

export type ScreenId = 'library' | 'profiles' | 'settings';

const SCREENS: readonly ScreenId[] = ['library', 'profiles', 'settings'];

export function Sidebar({ current, onSelect }: { current: ScreenId; onSelect: (screen: ScreenId) => void }) {
  const { t } = useI18n();
  return (
    <nav aria-label={t.nav.label} className="sidebar">
      {SCREENS.map((screen) => (
        <button
          key={screen}
          type="button"
          aria-current={current === screen ? 'page' : undefined}
          onClick={() => onSelect(screen)}
        >
          {t.nav[screen]}
        </button>
      ))}
    </nav>
  );
}
