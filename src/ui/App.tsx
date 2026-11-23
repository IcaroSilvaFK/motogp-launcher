import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import { readAppVersion, selectSessionClean, useLauncherState } from '../application';
import type { AppInfo, LauncherController, UpdateController } from '../application';
import { AppInfoContext } from './AppInfoContext';
import { I18nProvider, useI18n } from './i18n';
import type { Locale } from './i18n';
import { LauncherContext } from './LauncherContext';
import { LibraryScreen } from './LibraryScreen';
import { NoticeBar } from './NoticeBar';
import { ProfilesScreen } from './ProfilesScreen';
import { SettingsScreen } from './SettingsScreen';
import { StatusBar } from './StatusBar';
import { TopNav } from './TopNav';
import type { ScreenId } from './TopNav';
import { UpdateBanner } from './UpdateBanner';
import './styles.css';

function UpdateSlot({ updater, installBlocked }: { updater: UpdateController; installBlocked: boolean }) {
  const state = useSyncExternalStore(updater.subscribe, updater.getState);
  return <UpdateBanner state={state} installBlocked={installBlocked} onUpdate={() => void updater.install()} />;
}

/** Placeholder rows shown while the launcher data loads, so no screen flashes its empty state. */
function LoadingState() {
  const { t } = useI18n();
  return (
    <div className="loading-state">
      <p className="hint">{t.loading}</p>
      <div className="skeleton-list" aria-hidden="true">
        {[0, 1, 2, 3].map((row) => (
          <div key={row} className="skeleton-row">
            <span className="skeleton skeleton-tile" />
            <span className="skeleton skeleton-line" />
            <span className="skeleton skeleton-chip" />
          </div>
        ))}
      </div>
    </div>
  );
}

function Shell({
  controller,
  updater,
  appInfo,
}: {
  controller: LauncherController;
  updater?: UpdateController;
  appInfo?: AppInfo;
}) {
  const { t } = useI18n();
  const state = useLauncherState(controller);
  const [screen, setScreen] = useState<ScreenId>('library');
  const [version, setVersion] = useState<string | null>(null);
  const [userChecked, setUserChecked] = useState(false);
  const value = useMemo(() => ({ controller, state }), [controller, state]);
  const markUserChecked = useCallback(() => setUserChecked(true), []);
  const info = useMemo(
    () => ({ version, updater, userChecked, markUserChecked }),
    [version, updater, userChecked, markUserChecked],
  );
  const loading = state.status === 'loading';

  useEffect(() => {
    void controller.start();
    return () => controller.dispose();
  }, [controller]);

  // Read once per AppInfo port; a failure leaves the version hidden (readAppVersion never throws).
  useEffect(() => {
    if (!appInfo) return;
    let active = true;
    void readAppVersion(appInfo).then((read) => {
      if (active) setVersion(read);
    });
    return () => {
      active = false;
    };
  }, [appInfo]);

  return (
    <AppInfoContext.Provider value={info}>
      <LauncherContext.Provider value={value}>
        <div className="app-shell">
          <header className="topbar">
            <div className="brand">
              <svg className="brand-mark" viewBox="0 0 20 20" aria-hidden="true" focusable="false">
                <rect x="2" y="2" width="16" height="16" rx="4" />
                <path d="M6 13.5l2.6-7 1.4 3.6 1.4-3.6 2.6 7" />
              </svg>
              <h1>{t.appName}</h1>
            </div>
            <TopNav current={screen} onSelect={setScreen} />
          </header>
          <div className="app-messages">
            <NoticeBar />
            {updater && <UpdateSlot updater={updater} installBlocked={!selectSessionClean(state)} />}
          </div>
          <main aria-busy={loading ? 'true' : undefined}>
            {loading ? (
              <LoadingState />
            ) : (
              <>
                {screen === 'library' && <LibraryScreen />}
                {screen === 'profiles' && <ProfilesScreen />}
                {screen === 'settings' && <SettingsScreen />}
              </>
            )}
          </main>
          <StatusBar onSetUpGame={() => setScreen('settings')} />
        </div>
      </LauncherContext.Provider>
    </AppInfoContext.Provider>
  );
}

export function App({
  controller,
  locale,
  onLocaleChange,
  updater,
  appInfo,
}: {
  controller: LauncherController;
  locale?: Locale;
  onLocaleChange?: (locale: Locale) => void;
  updater?: UpdateController;
  appInfo?: AppInfo;
}) {
  return (
    <I18nProvider locale={locale} onLocaleChange={onLocaleChange}>
      <Shell controller={controller} updater={updater} appInfo={appInfo} />
    </I18nProvider>
  );
}
