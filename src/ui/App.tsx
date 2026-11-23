import { useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import { selectCanPlay, selectSessionClean, useLauncherState } from '../application';
import type { LauncherController, UpdateController } from '../application';
import { I18nProvider, useI18n } from './i18n';
import type { Locale } from './i18n';
import { LauncherContext } from './LauncherContext';
import { LibraryScreen } from './LibraryScreen';
import { NoticeBar } from './NoticeBar';
import { ProfilesScreen } from './ProfilesScreen';
import { SessionStatus } from './SessionStatus';
import { SettingsScreen } from './SettingsScreen';
import { Sidebar } from './Sidebar';
import type { ScreenId } from './Sidebar';
import { UpdateBanner } from './UpdateBanner';
import './styles.css';

function UpdateSlot({ updater, installBlocked }: { updater: UpdateController; installBlocked: boolean }) {
  const state = useSyncExternalStore(updater.subscribe, updater.getState);
  return <UpdateBanner state={state} installBlocked={installBlocked} onUpdate={() => void updater.install()} />;
}

function Shell({ controller, updater }: { controller: LauncherController; updater?: UpdateController }) {
  const { t } = useI18n();
  const state = useLauncherState(controller);
  const [screen, setScreen] = useState<ScreenId>('library');
  const value = useMemo(() => ({ controller, state }), [controller, state]);

  useEffect(() => {
    void controller.start();
    return () => controller.dispose();
  }, [controller]);

  return (
    <LauncherContext.Provider value={value}>
      <header className="app-header">
        <h1>{t.appName}</h1>
        <div className="field">
          <label htmlFor="active-profile">{t.activeProfile.label}</label>
          <select
            id="active-profile"
            value={state.config.active_profile ?? ''}
            onChange={(event) => void controller.setActiveProfile(event.target.value)}
          >
            {state.config.active_profile === null && <option value="" disabled />}
            {state.profiles.map((profile) => (
              <option key={profile.id} value={profile.id}>
                {profile.name}
              </option>
            ))}
          </select>
        </div>
        <button type="button" className="play" disabled={!selectCanPlay(state)} onClick={() => void controller.play()}>
          {t.play.button}
        </button>
        <SessionStatus />
      </header>
      <NoticeBar />
      {updater && <UpdateSlot updater={updater} installBlocked={!selectSessionClean(state)} />}
      <div className="layout">
        <Sidebar current={screen} onSelect={setScreen} />
        <main>
          {state.status === 'loading' ? <p>{t.loading}</p> : null}
          {screen === 'library' && <LibraryScreen />}
          {screen === 'profiles' && <ProfilesScreen />}
          {screen === 'settings' && <SettingsScreen />}
        </main>
      </div>
    </LauncherContext.Provider>
  );
}

export function App({
  controller,
  locale,
  onLocaleChange,
  updater,
}: {
  controller: LauncherController;
  locale?: Locale;
  onLocaleChange?: (locale: Locale) => void;
  updater?: UpdateController;
}) {
  return (
    <I18nProvider locale={locale} onLocaleChange={onLocaleChange}>
      <Shell controller={controller} updater={updater} />
    </I18nProvider>
  );
}
