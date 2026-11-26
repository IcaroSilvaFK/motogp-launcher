import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import {
  createModStatsStore,
  loadSidebarCollapsed,
  readAppVersion,
  saveSidebarCollapsed,
  selectSessionClean,
  useLauncherState,
} from '../application';
import type {
  AdPreferenceStore,
  AutoEnablePreferenceStore,
  AdProvider,
  CommunityAccount,
  DownloadStats,
  UsagePreferenceHandle,
  AppInfo,
  CatalogController,
  DiscoverViewStore,
  LauncherController,
  ModStatsSource,
  ModComments,
  ModPublisher,
  PublishFilePicker,
  NavPreferenceStore,
  UpdateController,
  WindowControls,
} from '../application';
import { AdConsentBar } from './AdConsentBar';
import { AdPreferenceProvider } from './AdPreferenceContext';
import { AutoEnablePreferenceProvider } from './AutoEnablePreferenceContext';
import { AdsContext } from './AdsContext';
import { CommunityContext } from './CommunityContext';
import { DownloadStatsProvider } from './DownloadStatsContext';
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
  autoEnableStore,
  adProvider,
  communityAccount,
  modStats,
  modComments,
  downloadStats,
  usagePreference,
  modPublisher,
  publishPicker,
}: {
  controller: LauncherController;
  adProvider?: AdProvider;
  communityAccount?: CommunityAccount;
  modStats?: ModStatsSource;
  modComments?: ModComments;
  /** Download counts and the 30-day chart (#120); absent means none render. */
  downloadStats?: DownloadStats;
  /** Anonymous usage sharing (#120); absent means no Settings row. */
  usagePreference?: UsagePreferenceHandle;
  /** Mod hosting (#130): publish and "My mods". Absent: no publish UI. */
  modPublisher?: ModPublisher;
  publishPicker?: PublishFilePicker;
  updater?: UpdateController;
  appInfo?: AppInfo;
  navStore?: NavPreferenceStore;
  windowControls?: WindowControls;
  catalog?: CatalogController;
  discoverViewStore?: DiscoverViewStore;
  adPreferenceStore?: AdPreferenceStore;
  /** "Enable mods after installing from Discover" (#129); without it the preference is on and not persisted. */
  autoEnableStore?: AutoEnablePreferenceStore;
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
  // One stars store per source, so Discover and the mod page share its 60 s cache (#119).
  const starsStore = useMemo(() => (modStats ? createModStatsStore(modStats) : undefined), [modStats]);
  const community = useMemo(
    () => ({
      account: communityAccount,
      stars: starsStore,
      comments: modComments,
      publisher: modPublisher,
      publishPicker,
    }),
    [communityAccount, starsStore, modComments, modPublisher, publishPicker],
  );

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
          <AutoEnablePreferenceProvider store={autoEnableStore}>
            <AdsContext.Provider value={ads}>
              <CommunityContext.Provider value={community}>
                <DownloadStatsProvider stats={downloadStats} usagePreference={usagePreference}>
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
                        <NoticeBar onOpenLibrary={() => setScreen('library')} />
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
                </DownloadStatsProvider>
              </CommunityContext.Provider>
            </AdsContext.Provider>
          </AutoEnablePreferenceProvider>
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
  autoEnableStore,
  adProvider,
  communityAccount,
  modStats,
  modComments,
  downloadStats,
  usagePreference,
  modPublisher,
  publishPicker,
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
  /** "Enable mods after installing from Discover" (#129); without it the preference is on and not persisted. */
  autoEnableStore?: AutoEnablePreferenceStore;
  /** Sponsored placements (#104); absent means no ads at all. Slots render only while the preference allows. */
  adProvider?: AdProvider;
  communityAccount?: CommunityAccount;
  /** Community stars (#119); absent means no counts and no star button. */
  modStats?: ModStatsSource;
  modComments?: ModComments;
  /** Download counts and the 30-day chart (#120); absent means none render. */
  downloadStats?: DownloadStats;
  /** Anonymous usage sharing (#120); absent means no Settings row. */
  usagePreference?: UsagePreferenceHandle;
  /** Mod hosting (#130): publish and "My mods". Absent: no publish UI. */
  modPublisher?: ModPublisher;
  publishPicker?: PublishFilePicker;
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
        autoEnableStore={autoEnableStore}
        adProvider={adProvider}
        communityAccount={communityAccount}
        modStats={modStats}
        modComments={modComments}
        downloadStats={downloadStats}
        usagePreference={usagePreference}
        modPublisher={modPublisher}
        publishPicker={publishPicker}
      />
    </I18nProvider>
  );
}
