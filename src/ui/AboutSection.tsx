import { useState, useSyncExternalStore } from 'react';
import { copyDiagnostics, selectSessionClean } from '../application';
import { AdSlot } from './AdSlot';
import type { Diagnostics, UpdateController, UpdateState } from '../application';
import { useAppInfo } from './AppInfoContext';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';
import { stagger } from './motion';
import { SupportLink } from './SupportLink';

/**
 * Settings > About: the launcher version, a manual update check when the updater exists, and the
 * "Support the project" row (#94).
 */
export function AboutSection() {
  const { t } = useI18n();
  const { version, updater, diagnostics } = useAppInfo();

  return (
    <section
      id="settings-about"
      aria-labelledby="settings-about-title"
      className="settings-section stagger-item"
      style={stagger(3)}
    >
      <h3 id="settings-about-title" className="group-title">
        {t.about.title}
      </h3>
      <div className="card rows">
        <div className="setting-row">
          <div className="setting-text">
            <p className="setting-label">{t.about.versionLabel}</p>
            <p className="hint">{t.about.versionHint}</p>
          </div>
          <div className="setting-control">
            {version === null ? (
              <span className="setting-value muted">{t.about.versionUnknown}</span>
            ) : (
              <span className="setting-value num">{version}</span>
            )}
          </div>
        </div>
        {updater && <UpdateCheckRow updater={updater} />}
        {diagnostics && <DiagnosticsRow diagnostics={diagnostics} />}
        <ShortcutsRow />
        <SupportLink placement="settings" />
        <AdSlot slot="about" />
      </div>
    </section>
  );
}

/** The keyboard shortcuts (#157), read-only. */
function ShortcutsRow() {
  const { t } = useI18n();
  const rows: [string, string][] = [
    ['Esc', t.about.shortcutEsc],
    ['/ · Ctrl+F', t.about.shortcutSearch],
    ['Ctrl+1…4', t.about.shortcutSections],
  ];
  return (
    <div className="setting-row">
      <div className="setting-text">
        <p className="setting-label">{t.about.shortcutsLabel}</p>
        <p className="hint">{t.about.shortcutsHint}</p>
      </div>
      <dl className="setting-control shortcut-list">
        {rows.map(([keys, label]) => (
          <div key={keys} className="shortcut-item">
            <dt>
              <kbd className="num">{keys}</kbd>
            </dt>
            <dd>{label}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

/** "Copy diagnostics" (#148): puts a bug-report text on the clipboard and confirms or fails inline. */
function DiagnosticsRow({ diagnostics }: { diagnostics: Diagnostics }) {
  const { t } = useI18n();
  const { state } = useLauncher();
  const { version } = useAppInfo();
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<'copied' | 'failed' | null>(null);

  const copy = async () => {
    setBusy(true);
    setResult(null);
    try {
      await copyDiagnostics(diagnostics, {
        version,
        gameDir: state.game?.root ?? null,
        modCount: state.mods.length,
        activeProfile: state.profiles.find((profile) => profile.id === state.config.active_profile)?.name ?? null,
      });
      setResult('copied');
    } catch (error) {
      console.warn('Could not copy the diagnostics', error);
      setResult('failed');
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="setting-row">
      <div className="setting-text">
        <p className="setting-label">{t.about.diagnosticsLabel}</p>
        <p className="hint">{t.about.diagnosticsHint}</p>
        <div role="status">
          {result === 'copied' && (
            <span className="update-result-ok">
              <Icon name="check" />
              {t.about.copied}
            </span>
          )}
          {result === 'failed' && <span className="update-result-error">{t.about.copyFailed}</span>}
        </div>
      </div>
      <div className="setting-control">
        <button type="button" className="btn btn-sm" disabled={busy} onClick={() => void copy()}>
          <Icon name="file" />
          {t.about.copyDiagnostics}
        </button>
      </div>
    </div>
  );
}

function UpdateCheckRow({ updater }: { updater: UpdateController }) {
  const { t } = useI18n();
  const { state: launcher } = useLauncher();
  const { userChecked, markUserChecked } = useAppInfo();
  const state = useSyncExternalStore(updater.subscribe, updater.getState);
  const [checking, setChecking] = useState(false);

  const busy = state.kind === 'downloading' || state.kind === 'installed';
  const info = state.kind === 'available' || state.kind === 'error' ? state.info : null;
  const sessionBlocked = !selectSessionClean(launcher);

  const check = async () => {
    setChecking(true);
    try {
      await updater.check();
    } finally {
      setChecking(false);
      markUserChecked();
    }
  };

  return (
    <div className="setting-row setting-row-block setting-row-stack">
      <div className="setting-row-line">
        <div className="setting-text">
          <p className="setting-label">{t.about.updatesLabel}</p>
          <p className="hint">{t.about.updatesHint}</p>
        </div>
        <div className="setting-control">
          <button type="button" className="btn btn-sm" disabled={checking || busy} onClick={() => void check()}>
            <Icon name="refresh" />
            {checking ? t.about.checking : t.about.check}
          </button>
        </div>
      </div>

      <div className="update-result">
        <div role="status" className="update-result-line">
          {!checking && <ResultText state={state} userChecked={userChecked} />}
        </div>
        {info !== null && !busy && !checking && (
          <div className="update-result-action">
            <button
              type="button"
              className="btn btn-sm btn-accent-outline"
              disabled={sessionBlocked}
              aria-describedby={sessionBlocked ? 'about-update-blocked' : undefined}
              onClick={() => void updater.install()}
            >
              <Icon name="import" />
              {t.about.install}
            </button>
            {sessionBlocked && (
              <p id="about-update-blocked" className="hint">
                {t.update.blockedBySession}
              </p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function ResultText({ state, userChecked }: { state: UpdateState; userChecked: boolean }) {
  const { t } = useI18n();
  switch (state.kind) {
    case 'none':
      return userChecked ? (
        <span className="update-result-ok">
          <Icon name="check" />
          {t.about.upToDate}
        </span>
      ) : null;
    case 'available':
      return <span className="update-result-new">{t.about.available(state.info.version)}</span>;
    case 'downloading':
      return <span>{t.update.downloading}</span>;
    case 'installed':
      return <span>{t.update.restarting}</span>;
    case 'error':
      return (
        <>
          {state.info && <span className="update-result-new">{t.about.available(state.info.version)}</span>}
          <span className="update-result-error">{t.update.errors[state.error.kind]}</span>
        </>
      );
    default:
      return null;
  }
}
