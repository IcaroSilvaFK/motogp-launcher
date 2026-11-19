import { LOCALES, useI18n } from './i18n';
import type { Locale } from './i18n';
import { useLauncher } from './LauncherContext';

export function SettingsScreen() {
  const { t, locale, setLocale } = useI18n();
  const { controller, state } = useLauncher();
  const { game, detected } = state;

  return (
    <section aria-labelledby="settings-title">
      <h2 id="settings-title">{t.settings.title}</h2>

      <h3>{t.settings.game}</h3>
      {game === null ? (
        <p>{t.settings.noGame}</p>
      ) : (
        <dl>
          <dt>{t.settings.game}</dt>
          <dd>{game.name}</dd>
          <dt>{t.settings.modsDir}</dt>
          <dd>{game.mods_dir}</dd>
          <dt>{t.settings.appId}</dt>
          <dd>{game.app_id === null ? t.settings.appIdNotFound : String(game.app_id)}</dd>
        </dl>
      )}

      <div className="actions">
        <button type="button" onClick={() => void controller.detectGames()}>
          {t.settings.detect}
        </button>
        <button type="button" onClick={() => void controller.chooseGameDirectory()}>
          {t.settings.chooseFolder}
        </button>
      </div>

      {detected !== null && (
        <>
          <h3>{t.settings.detections}</h3>
          {detected.length === 0 ? (
            <p>{t.settings.noDetections}</p>
          ) : (
            <ul>
              {detected.map((candidate) => (
                <li key={candidate.root}>
                  <span>
                    {candidate.name} — {candidate.root}
                  </span>{' '}
                  <button type="button" onClick={() => void controller.useGame(candidate.root)}>
                    {t.settings.useThis}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </>
      )}

      <div className="field">
        <label htmlFor="language-select">{t.settings.language}</label>
        <select id="language-select" value={locale} onChange={(event) => setLocale(event.target.value as Locale)}>
          {LOCALES.map((option) => (
            <option key={option} value={option}>
              {t.settings.languageNames[option]}
            </option>
          ))}
        </select>
      </div>
    </section>
  );
}
