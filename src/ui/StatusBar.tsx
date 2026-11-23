import { selectCanPlay } from '../application';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';
import { SessionStatus } from './SessionStatus';

/**
 * Persistent bottom bar: the configured game on the left, the session status in the middle and the
 * active profile with the Play action on the right.
 */
export function StatusBar({ onSetUpGame }: { onSetUpGame: () => void }) {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const { game } = state;
  const canPlay = selectCanPlay(state);

  return (
    <footer className="statusbar">
      <div className="statusbar-game">
        <span className="game-tile" aria-hidden="true">
          <Icon name="game" />
        </span>
        {game === null ? (
          <div className="statusbar-game-text">
            <span className="statusbar-game-name muted">{t.statusBar.noGame}</span>
            <button type="button" className="btn-link" onClick={onSetUpGame}>
              {t.statusBar.setUp}
            </button>
          </div>
        ) : (
          <div className="statusbar-game-text">
            <span className="statusbar-game-name" title={game.root}>
              {game.name}
            </span>
            {game.app_id !== null && <span className="badge badge-store">{t.statusBar.steam}</span>}
          </div>
        )}
      </div>

      <div className="statusbar-message">
        <SessionStatus fallback={canPlay ? <p className="statusbar-idle">{t.statusBar.ready}</p> : null} />
      </div>

      <div className="statusbar-actions">
        <div className="field profile-field">
          <label htmlFor="active-profile">{t.activeProfile.label}</label>
          <select
            id="active-profile"
            className="input"
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
        <button
          type="button"
          className="btn btn-primary play"
          disabled={!canPlay}
          onClick={() => void controller.play()}
        >
          <Icon name="play" className="icon play-icon" />
          {t.play.button}
        </button>
      </div>
    </footer>
  );
}
