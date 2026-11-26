import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import {
  loadSidebarCollapsed,
  readAppVersion,
  saveSidebarCollapsed,
  selectSessionClean,
  useLauncherState,
} from '../application';
import type {
  AdPreferenceStore,
  AdProvider,
  CommunityAccount,
  AppInfo,
  CatalogController,
  DiscoverViewStore,
  LauncherController,
  NavPreferenceStore,
  UpdateController,
  WindowControls,
} from '../application';
import { AdConsentBar } from './AdConsentBar';
import { AdPreferenceProvider } from './AdPreferenceContext';
import { AdsContext } from './AdsContext';
import { CommunityContext } from './CommunityContext';
import { AppInfoContext } from './AppInfoContext';
import { DiscoverScreen } from './DiscoverScreen';
import { I18nProvider, useI18n } from './i18n';
import type { Locale } from './i18n';
import { LauncherContext } from './LauncherContext';
import { LibraryScreen } from './LibraryScreen';
import { NoticeBar } from './NoticeBar';
import { ProfilesScreen } from './ProfilesScreen';
import { SettingsScreen } from './SettingsScreen';
import { ALL_SCREENS, Sidebar } from './Sidebar';
import type { ScreenId } from './Sidebar';
import { ScreenTransition } from './ScreenTransition';
import { StatusBar } from './StatusBar';
import { TitleBar } from './TitleBar';
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
  navStore,
  windowControls,
  catalog,
  discoverViewStore,
  adPreferenceStore,
  adProvider,
  communityAccount,
}: {
  controller: LauncherController;
  adProvider?: AdProvider;
  communityAccount?: CommunityAccount;
  updater?: UpdateController;
  appInfo?: AppInfo;
  navStore?: NavPreferenceStore;
  windowControls?: WindowControls;
  catalog?: CatalogController;
  discoverViewStore?: DiscoverViewStore;
  adPreferenceStore?: AdPreferenceStore;
}) {
  const state = useLauncherState(controller);
  const screens = useMemo(() => (catalog ? ALL_SCREENS : ALL_SCREENS.filter((id) => id !== 'discover')), [catalog]);
  const [screen, setScreen] = useState<ScreenId>('library');
  // Read once on mount; without a store the choice lives in memory and starts expanded.
  const [collapsed, setCollapsed] = useState(() => (navStore ? loadSidebarCollapsed(navStore) : false));
  // The save stays out of the state updater, which StrictMode may run twice.
  const toggleSidebar = useCallback(() => {
    const next = !collapsed;
    setCollapsed(next);
    if (navStore) saveSidebarCollapsed(navStore, next);
  }, [collapsed, navStore]);
  const [version, setVersion] = useState<string | null>(null);
  const [userChecked, setUserChecked] = useState(false);
  const value = useMemo(() => ({ controller, state }), [controller, state]);
  const markUserChecked = useCallback(() => setUserChecked(true), []);
  const info = useMemo(
    () => ({ version, updater, userChecked, markUserChecked }),
    [version, updater, userChecked, markUserChecked],
  );
  const loading = state.status === 'loading';
  const ads = useMemo(() => ({ provider: adProvider }), [adProvider]);
  const community = useMemo(() => ({ account: communityAccount }), [communityAccount]);

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
        <AdPreferenceProvider store={adPreferenceStore}>
          <AdsContext.Provider value={ads}>
            <CommunityContext.Provider value={community}>
              <div className="app-shell">
                <TitleBar controls={windowControls} />
                <Sidebar
                  screens={screens}
                  current={screen}
                  onSelect={setScreen}
                  collapsed={collapsed}
                  onToggle={toggleSidebar}
                />
                <div className="app-content">
                  <div className="app-messages">
                    <NoticeBar />
                    <div className="notices">
                      <AdConsentBar />
                    </div>
                    {updater && <UpdateSlot updater={updater} installBlocked={!selectSessionClean(state)} />}
                  </div>
                  <main aria-busy={loading ? 'true' : undefined}>
                    {loading ? (
                      <LoadingState />
                    ) : (
                      // Keyed by screen: each switch mounts a fresh wrapper, which plays the enter animation.
                      <ScreenTransition key={screen}>
                        {screen === 'library' && <LibraryScreen />}
                        {screen === 'discover' && catalog && (
                          <DiscoverScreen
                            catalog={catalog}
                            onGoToLibrary={() => setScreen('library')}
                            viewStore={discoverViewStore}
                          />
                        )}
                        {screen === 'profiles' && <ProfilesScreen />}
                        {screen === 'settings' && <SettingsScreen />}
                      </ScreenTransition>
                    )}
                  </main>
                </div>
                <StatusBar onSetUpGame={() => setScreen('settings')} />
              </div>
            </CommunityContext.Provider>
          </AdsContext.Provider>
        </AdPreferenceProvider>
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
  navStore,
  windowControls,
  catalog,
  discoverViewStore,
  adPreferenceStore,
  adProvider,
  communityAccount,
}: {
  controller: LauncherController;
  locale?: Locale;
  onLocaleChange?: (locale: Locale) => void;
  updater?: UpdateController;
  appInfo?: AppInfo;
  navStore?: NavPreferenceStore;
  windowControls?: WindowControls;
  /** The Discover tab (#73) is listed only when a catalog controller is wired. */
  catalog?: CatalogController;
  discoverViewStore?: DiscoverViewStore;
  /** The sponsored-content choice (#105); without it ads stay off. */
  adPreferenceStore?: AdPreferenceStore;
  /** Sponsored placements (#104); absent means no ads at all. Slots render only while the preference allows. */
  adProvider?: AdProvider;
  communityAccount?: CommunityAccount;
}) {
  return (
    <I18nProvider locale={locale} onLocaleChange={onLocaleChange}>
      <Shell
        controller={controller}
        updater={updater}
        appInfo={appInfo}
        navStore={navStore}
        windowControls={windowControls}
        catalog={catalog}
        discoverViewStore={discoverViewStore}
        adPreferenceStore={adPreferenceStore}
        adProvider={adProvider}
        communityAccount={communityAccount}
      />
    </I18nProvider>
  );
}
