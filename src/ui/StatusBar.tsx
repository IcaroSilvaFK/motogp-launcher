import { useEffect, useId, useRef, useState } from 'react';
import { selectCanPlay, selectCanStop } from '../application';
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
  const { state } = useLauncher();
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
        <SessionAction />
      </div>
    </footer>
  );
}

/**
 * The status bar's main action: Play, Stop while the game can be stopped (behind an inline
 * confirmation), or a disabled progress label while it stops or cleans up.
 */
function SessionAction() {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const { session } = state;
  const canPlay = selectCanPlay(state);
  const canStop = selectCanStop(state);
  const [confirming, setConfirming] = useState(false);
  // The game can end while the question is open; the confirmation then closes by itself.
  if (confirming && !canStop) setConfirming(false);
  const showConfirm = confirming && canStop;

  const actionRef = useRef<HTMLButtonElement>(null);
  const confirmRef = useRef<HTMLButtonElement>(null);
  const wasConfirming = useRef(false);
  const titleId = useId();
  const detailId = useId();

  // Focus goes to the confirm action on open and back to the main action when the question closes
  // (cancelled, confirmed or outlived), so it is never dropped on the page.
  useEffect(() => {
    if (showConfirm) confirmRef.current?.focus();
    else if (wasConfirming.current) actionRef.current?.focus();
    wasConfirming.current = showConfirm;
  }, [showConfirm]);

  if (showConfirm) {
    return (
      <div
        className="session-confirm"
        role="group"
        aria-labelledby={titleId}
        aria-describedby={detailId}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.preventDefault();
            setConfirming(false);
          }
        }}
      >
        <p className="session-confirm-text">
          <strong id={titleId}>{t.stop.confirmTitle}</strong>
          <span id={detailId}>{t.stop.confirmDetail}</span>
        </p>
        {/* Cancel sits where Stop was, so a double click on Stop never confirms by accident. */}
        <button
          ref={confirmRef}
          type="button"
          className="btn btn-danger"
          onClick={() => {
            setConfirming(false);
            void controller.stopSession();
          }}
        >
          {t.stop.confirm}
        </button>
        <button type="button" className="btn btn-ghost" onClick={() => setConfirming(false)}>
          {t.stop.cancel}
        </button>
      </div>
    );
  }

  const profile = <ActiveProfileField />;

  if (canStop) {
    return (
      <>
        {profile}
        <button ref={actionRef} type="button" className="btn btn-danger play" onClick={() => setConfirming(true)}>
          <Icon name="stop" className="icon" />
          {t.stop.button}
        </button>
      </>
    );
  }

  if (session.kind === 'stopping' || session.kind === 'cleaning-up') {
    // aria-disabled (not disabled) keeps the focus here after confirming instead of losing it.
    return (
      <>
        {profile}
        <button ref={actionRef} type="button" className="btn play play-progress" aria-disabled="true">
          {session.kind === 'stopping' ? t.stop.stopping : t.stop.cleaningUp}
        </button>
      </>
    );
  }

  return (
    <>
      {profile}
      <button
        ref={actionRef}
        type="button"
        className="btn btn-primary play"
        disabled={!canPlay}
        onClick={() => void controller.play()}
      >
        <Icon name="play" className="icon play-icon" />
        {t.play.button}
      </button>
    </>
  );
}

function ActiveProfileField() {
  const { t } = useI18n();
  const { controller, state } = useLauncher();

  return (
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
  );
}
