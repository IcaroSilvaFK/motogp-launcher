import { useState } from 'react';
import type { MouseEvent } from 'react';
import { AboutSection } from './AboutSection';
import { useAdPreference } from './AdPreferenceContext';
import { useAutoEnablePreference } from './AutoEnablePreferenceContext';
import { CommunityAccountSection } from './CommunityAccountSection';
import { useCommunity } from './CommunityContext';
import { Icon } from './Icon';
import type { IconName } from './Icon';
import { LOCALES, useI18n } from './i18n';
import type { Locale } from './i18n';
import { useLauncher } from './LauncherContext';
import { stagger } from './motion';
import { StorageSection } from './StorageSection';
import { UsageSharingRow } from './UsageSharingRow';

type SectionId = 'game' | 'preferences' | 'community' | 'storage' | 'about';

const SECTIONS: readonly { id: SectionId; icon: IconName }[] = [
  { id: 'game', icon: 'game' },
  { id: 'preferences', icon: 'language' },
  { id: 'community', icon: 'account' },
  { id: 'storage', icon: 'storage' },
  { id: 'about', icon: 'info' },
];

export function SettingsScreen() {
  const { t, locale, setLocale } = useI18n();
  const { controller, state } = useLauncher();
  const { preference: adPreference, setPreference: setAdPreference } = useAdPreference();
  const { enabled: autoEnable, setEnabled: setAutoEnable } = useAutoEnablePreference();
  // Without a community port (tests, builds without the community) the section and its link are absent.
  const hasCommunity = useCommunity().account !== undefined;
  const sections = SECTIONS.filter(({ id }) => id !== 'community' || hasCommunity);
  const { game, detected } = state;
  const [current, setCurrent] = useState<SectionId>('game');
  const sectionTitle: Record<SectionId, string> = {
    game: t.settings.game,
    preferences: t.settings.preferences,
    community: t.community.title,
    storage: t.storage.title,
    about: t.about.title,
  };

  const jump = (event: MouseEvent<HTMLAnchorElement>, id: SectionId) => {
    event.preventDefault();
    setCurrent(id);
    document.getElementById(`settings-${id}`)?.scrollIntoView?.({ block: 'start' });
  };

  return (
    <section aria-labelledby="settings-title" className="page">
      <div className="page-head">
        <div className="page-heading">
          <h2 id="settings-title">{t.settings.title}</h2>
          <p className="page-subtitle">{t.settings.subtitle}</p>
        </div>
      </div>

      <div className="split">
        <nav aria-label={t.settings.sectionsLabel} className="section-nav split-rail">
          {sections.map(({ id, icon }) => (
            <a
              key={id}
              href={`#settings-${id}`}
              className="section-link"
              aria-current={current === id ? 'location' : undefined}
              onClick={(event) => jump(event, id)}
            >
              <Icon name={icon} className="nav-icon" />
              {sectionTitle[id]}
            </a>
          ))}
        </nav>

        <div className="split-main settings-sections">
          <section
            id="settings-game"
            aria-labelledby="settings-game-title"
            className="settings-section stagger-item"
            style={stagger(0)}
          >
            <h3 id="settings-game-title" className="group-title">
              {t.settings.game}
            </h3>
            <div className="card rows">
              <div className="setting-row">
                <div className="setting-text">
                  <p className="setting-label">{t.settings.folderTitle}</p>
                  <p className="hint">{t.settings.gameHint}</p>
                </div>
                <div className="setting-control">
                  <button type="button" className="btn btn-sm" onClick={() => void controller.detectGames()}>
                    <Icon name="refresh" />
                    {t.settings.detect}
                  </button>
                  <button type="button" className="btn btn-sm" onClick={() => void controller.chooseGameDirectory()}>
                    <Icon name="folder" />
                    {t.settings.chooseFolder}
                  </button>
                </div>
              </div>

              <div className="setting-row setting-row-block">
                {game === null ? (
                  <p className="muted">{t.settings.noGame}</p>
                ) : (
                  <dl className="details">
                    <dt>{t.settings.game}</dt>
                    <dd>{game.name}</dd>
                    <dt>{t.settings.modsDir}</dt>
                    <dd>{game.mods_dir}</dd>
                    <dt>{t.settings.appId}</dt>
                    <dd>{game.app_id === null ? t.settings.appIdNotFound : String(game.app_id)}</dd>
                  </dl>
                )}
              </div>

              {detected !== null && (
                <div className="setting-row setting-row-block">
                  <h4 className="setting-label">{t.settings.detections}</h4>
                  {detected.length === 0 ? (
                    <p className="hint">{t.settings.noDetections}</p>
                  ) : (
                    <ul className="plain detected-list">
                      {detected.map((candidate) => (
                        <li key={candidate.root}>
                          <span className="detected-name">
                            {candidate.name} — {candidate.root}
                          </span>
                          <button
                            type="button"
                            className="btn btn-sm"
                            onClick={() => void controller.useGame(candidate.root)}
                          >
                            {t.settings.useThis}
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              )}
            </div>
          </section>

          <section
            id="settings-preferences"
            aria-labelledby="settings-preferences-title"
            className="settings-section stagger-item"
            style={stagger(1)}
          >
            <h3 id="settings-preferences-title" className="group-title">
              {t.settings.preferences}
            </h3>
            <div className="card rows">
              <div className="setting-row">
                <div className="setting-text">
                  <label htmlFor="language-select" className="setting-label">
                    {t.settings.language}
                  </label>
                  <p className="hint">{t.settings.languageHint}</p>
                </div>
                <div className="setting-control">
                  <select
                    id="language-select"
                    className="input"
                    value={locale}
                    onChange={(event) => setLocale(event.target.value as Locale)}
                  >
                    {LOCALES.map((option) => (
                      <option key={option} value={option}>
                        {t.settings.languageNames[option]}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
              <div className="setting-row">
                <div className="setting-text">
                  <label htmlFor="ad-preference-select" className="setting-label">
                    {t.settings.adPreference}
                  </label>
                  <p className="hint">{t.settings.adPreferenceHint}</p>
                </div>
                <div className="setting-control">
                  <select
                    id="ad-preference-select"
                    className="input"
                    value={adPreference === 'off' ? 'off' : 'on'}
                    onChange={(event) => setAdPreference(event.target.value === 'off' ? 'off' : 'on')}
                  >
                    <option value="on">{t.settings.adShow}</option>
                    <option value="off">{t.settings.adOff}</option>
                  </select>
                </div>
              </div>
              <div className="setting-row">
                <div className="setting-text">
                  <label htmlFor="auto-enable-checkbox" className="setting-label">
                    {t.settings.autoEnable}
                  </label>
                  <p className="hint">{t.settings.autoEnableHint}</p>
                </div>
                <div className="setting-control">
                  <input
                    id="auto-enable-checkbox"
                    type="checkbox"
                    checked={autoEnable}
                    onChange={(event) => setAutoEnable(event.target.checked)}
                  />
                </div>
              </div>
              <UsageSharingRow />
            </div>
          </section>

          <CommunityAccountSection />

          <StorageSection />

          <AboutSection />
        </div>
      </div>
    </section>
  );
}
