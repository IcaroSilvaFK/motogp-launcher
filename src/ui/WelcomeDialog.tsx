import { useEffect, useRef, useState } from 'react';
import type { KeyboardEvent, ReactNode } from 'react';
import { Icon } from './Icon';
import type { IconName } from './Icon';
import { useI18n } from './i18n';
import type { Messages } from './i18n';
import { useLauncher } from './LauncherContext';
import { errorText } from './NoticeBar';
import type { ScreenId } from './Sidebar';

const TOTAL = 3;

/** One step's layout: the title (focused on entry), its body and the footer actions. */
function StepFrame({ title, children, actions }: { title: string; children: ReactNode; actions: ReactNode }) {
  const titleRef = useRef<HTMLHeadingElement>(null);
  // Each step mounts its own frame, so the new title takes the focus and is announced.
  useEffect(() => {
    titleRef.current?.focus();
  }, []);
  return (
    <>
      <h2 id="welcome-title" ref={titleRef} tabIndex={-1} className="welcome-title">
        {title}
      </h2>
      <div className="welcome-body">{children}</div>
      <div className="welcome-actions">{actions}</div>
    </>
  );
}

/** Step 1: the configured or detected game folder to confirm, or a folder picker when none is found. */
function GameStep({ onNext }: { onNext: () => void }) {
  const { t } = useI18n();
  const { controller, state } = useLauncher();
  const { game, detected, error, busy } = state;
  // Detection runs once, only when no game is configured and it never ran (reuses detect_games).
  const [searching, setSearching] = useState(game === null && detected === null);
  const started = useRef(false);
  // Errors are shown only after an action of this step, never one left over from the startup.
  const [attempted, setAttempted] = useState(false);

  useEffect(() => {
    if (!searching || started.current) return;
    started.current = true;
    void controller.detectGames().finally(() => setSearching(false));
  }, [controller, searching]);

  const candidate = game ?? detected?.[0] ?? null;

  const confirm = async () => {
    if (game !== null) {
      onNext();
      return;
    }
    if (candidate === null) return;
    setAttempted(true);
    await controller.useGame(candidate.root);
    if (controller.getState().game !== null) onNext();
  };

  const pick = () => {
    setAttempted(true);
    void controller.chooseGameDirectory();
  };

  const failure = attempted && error !== null && (
    <p role="alert" className="welcome-error">
      <Icon name="info" />
      {errorText(t, error.code)}
    </p>
  );

  if (searching) {
    return (
      <StepFrame title={t.welcome.gameTitle} actions={null}>
        <p role="status" className="welcome-status">
          <Icon name="refresh" className="icon welcome-spin" />
          {t.welcome.detecting}
        </p>
      </StepFrame>
    );
  }

  if (candidate === null) {
    return (
      <StepFrame
        title={t.welcome.gameTitle}
        actions={
          <>
            <button type="button" className="btn btn-ghost" onClick={onNext}>
              {t.welcome.later}
            </button>
            <button type="button" className="btn btn-accent-outline" disabled={busy} onClick={pick}>
              <Icon name="folder" />
              {t.welcome.chooseFolder}
            </button>
          </>
        }
      >
        <p className="welcome-text">{t.welcome.notFoundBody}</p>
        {failure}
      </StepFrame>
    );
  }

  return (
    <StepFrame
      title={t.welcome.gameTitle}
      actions={
        <>
          <button type="button" className="btn" disabled={busy} onClick={pick}>
            <Icon name="folder" />
            {t.welcome.chooseOther}
          </button>
          <button type="button" className="btn btn-accent-outline" disabled={busy} onClick={() => void confirm()}>
            <Icon name="check" />
            {t.welcome.looksRight}
          </button>
        </>
      }
    >
      <p className="welcome-text">{t.welcome.foundBody}</p>
      <div className="welcome-path">
        <Icon name="game" />
        <div className="welcome-path-text">
          <span className="welcome-path-name">{candidate.name}</span>
          <code title={candidate.root}>{candidate.root}</code>
        </div>
      </div>
      {failure}
    </StepFrame>
  );
}

type WelcomeMessages = Messages['welcome'];
/** The plain-text keys of the welcome messages (not the `step` formatter). */
type WelcomeKey = {
  [K in keyof WelcomeMessages]: WelcomeMessages[K] extends string ? K : never;
}[keyof WelcomeMessages];

const MOD_STEPS: readonly { icon: IconName; title: WelcomeKey; text: WelcomeKey }[] = [
  { icon: 'import', title: 'installTitle', text: 'installText' },
  { icon: 'profiles', title: 'enableTitle', text: 'enableText' },
  { icon: 'play', title: 'playTitle', text: 'playText' },
];

/**
 * First-run welcome (#147): a modal dialog in three steps. Esc and Skip close it; both, like
 * finishing, count as seen. Focus stays inside and goes back to where it was on close.
 */
export function WelcomeDialog({
  hasCatalog,
  onClose,
}: {
  /** Without the online catalog the last step leads to the Library instead of Discover. */
  hasCatalog: boolean;
  /** Called on skip (no target) or finish (the screen to open). */
  onClose: (goTo?: ScreenId) => void;
}) {
  const { t } = useI18n();
  const [step, setStep] = useState(1);
  const dialogRef = useRef<HTMLDivElement>(null);

  // Gives the focus back to the control that had it (the Settings button, or nothing at launch).
  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    return () => {
      if (previous?.isConnected) previous.focus();
    };
  }, []);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== 'Tab' || dialogRef.current === null) return;
    const focusable = [...dialogRef.current.querySelectorAll<HTMLElement>('button:not([disabled])')];
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    const active = document.activeElement;
    const onEdgeStart = active === first || !focusable.includes(active as HTMLElement);
    if (event.shiftKey && onEdgeStart) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && active === last) {
      event.preventDefault();
      first?.focus();
    }
  };

  const back = (
    <button type="button" className="btn btn-ghost welcome-back" onClick={() => setStep(step - 1)}>
      <Icon name="chevron-left" />
      {t.welcome.back}
    </button>
  );

  return (
    <div className="welcome">
      <div className="welcome-scrim" aria-hidden="true" />
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={t.welcome.label}
        className="welcome-dialog"
        onKeyDown={onKeyDown}
      >
        <div className="welcome-head">
          <p className="welcome-step">{t.welcome.step(step, TOTAL)}</p>
          <button type="button" className="btn btn-ghost btn-sm" onClick={() => onClose()}>
            {t.welcome.skip}
          </button>
        </div>
        <div className="welcome-progress" aria-hidden="true">
          {[1, 2, 3].map((index) => (
            <span key={index} className="welcome-progress-segment" data-active={index <= step ? 'true' : undefined} />
          ))}
        </div>

        {step === 1 && <GameStep onNext={() => setStep(2)} />}

        {step === 2 && (
          <StepFrame
            title={t.welcome.modsTitle}
            actions={
              <>
                {back}
                <button type="button" className="btn btn-accent-outline" onClick={() => setStep(3)}>
                  {t.welcome.next}
                  <Icon name="chevron-right" />
                </button>
              </>
            }
          >
            <p className="welcome-text">{t.welcome.modsBody}</p>
            <ol className="plain welcome-steps">
              {MOD_STEPS.map(({ icon, title, text }, index) => (
                <li key={title} className="welcome-steps-item">
                  <span className="welcome-steps-icon" aria-hidden="true">
                    <Icon name={icon} />
                  </span>
                  <div>
                    <h3 className="welcome-steps-title">
                      <span className="welcome-steps-number num">{index + 1}.</span> {t.welcome[title]}
                    </h3>
                    <p className="hint">{t.welcome[text]}</p>
                  </div>
                </li>
              ))}
            </ol>
          </StepFrame>
        )}

        {step === 3 && (
          <StepFrame
            title={t.welcome.doneTitle}
            actions={
              <>
                {back}
                <button
                  type="button"
                  className="btn btn-accent-outline"
                  onClick={() => onClose(hasCatalog ? 'discover' : 'library')}
                >
                  <Icon name={hasCatalog ? 'discover' : 'library'} />
                  {hasCatalog ? t.welcome.goToDiscover : t.welcome.goToLibrary}
                </button>
              </>
            }
          >
            <p className="welcome-text">{hasCatalog ? t.welcome.doneBody : t.welcome.doneBodyLibrary}</p>
          </StepFrame>
        )}
      </div>
    </div>
  );
}
