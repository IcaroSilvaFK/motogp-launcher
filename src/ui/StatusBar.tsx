import { useState } from 'react';
import { selectCanPlay } from '../application';
import { useAppInfo } from './AppInfoContext';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';
import { SessionStatus } from './SessionStatus';

/**
 * Decorative game tile: the Steam artwork when there is one that loads, the generic game icon
 * otherwise. The failed URL is remembered (not a boolean), so a new artwork gets a fresh try.
 */
function GameTile({ artwork }: { artwork: string | null }) {
  const [failed, setFailed] = useState<string | null>(null);
  const showArtwork = artwork !== null && artwork !== failed;

  return (
    <span className={showArtwork ? 'game-tile has-artwork' : 'game-tile'} aria-hidden="true">
      {showArtwork ? (
        <img className="game-tile-image" src={artwork} alt="" draggable={false} onError={() => setFailed(artwork)} />
      ) : (
        <Icon name="game" />
      )}
    </span>
  );
}

/**
 * Persistent bottom bar: the configured game on the left, the session status in the middle and the
 * active profile with the Play action on the right.
 */
export function StatusBar({ onSetUpGame }: { onSetUpGame: () => void }) {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const { game } = state;
  const canPlay = selectCanPlay(state);
  const { version } = useAppInfo();

  return (
    <footer className="statusbar">
      <div className="statusbar-game">
        <GameTile artwork={game === null ? null : state.artwork} />
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
        {version !== null && (
          <span className="statusbar-version num" title={t.statusBar.versionTitle}>
            {t.statusBar.version(version)}
          </span>
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
