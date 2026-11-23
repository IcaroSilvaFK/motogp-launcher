import { useState } from 'react';
import type { MouseEvent } from 'react';
import { Icon } from './Icon';
import type { IconName } from './Icon';
import { LOCALES, useI18n } from './i18n';
import type { Locale } from './i18n';
import { useLauncher } from './LauncherContext';

type SectionId = 'game' | 'preferences';

const SECTIONS: readonly { id: SectionId; icon: IconName }[] = [
  { id: 'game', icon: 'game' },
  { id: 'preferences', icon: 'language' },
];

export function SettingsScreen() {
  const { t, locale, setLocale } = useI18n();
  const { controller, state } = useLauncher();
  const { game, detected } = state;
  const [current, setCurrent] = useState<SectionId>('game');
  const sectionTitle: Record<SectionId, string> = { game: t.settings.game, preferences: t.settings.preferences };

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
          {SECTIONS.map(({ id, icon }) => (
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
          <section id="settings-game" aria-labelledby="settings-game-title" className="settings-section">
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

          <section id="settings-preferences" aria-labelledby="settings-preferences-title" className="settings-section">
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
            </div>
          </section>
        </div>
      </div>
    </section>
  );
}
