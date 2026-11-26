import { useEffect, useId, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { useCommunityAccount, useModStats } from '../application';
import type { ToggleStarResult } from '../application';
import { useCommunity } from './CommunityContext';
import { Icon } from './Icon';
import { useI18n } from './i18n';

/**
 * A read-only star count (#119): the number for the eye, "12 stars" for screen readers. On the
 * Discover cards it carries a small star glyph; next to the star button the button's glyph is enough.
 */
export function StarCount({ stars, id, icon = true }: { stars: number; id?: string; icon?: boolean }) {
  const { t } = useI18n();
  return (
    <span id={id} className="star-count">
      {icon && <Icon name="star" className="icon star-count-icon" />}
      <span className="num" aria-hidden="true">
        {stars}
      </span>
      <span className="visually-hidden">{t.stars.count(stars)}</span>
    </span>
  );
}

/**
 * The mod page's star toggle (#119): star glyph + label, `aria-pressed`, and the count beside it.
 * Hidden while the stats load and when the community is out of reach. Signed out, a click opens a
 * small inline prompt to sign in with Discord; once signed in the star is applied.
 */
export function StarButton({ modId }: { modId: string }) {
  const { t } = useI18n();
  const { account, stars: store } = useCommunity();
  const snapshot = useModStats(store);
  const community = useCommunityAccount(account);
  const [prompt, setPrompt] = useState(false);
  const [wantStar, setWantStar] = useState(false);
  const [failure, setFailure] = useState<'failed' | 'banned' | null>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const signInRef = useRef<HTMLButtonElement>(null);
  const countId = useId();
  const promptId = useId();

  // Bumped when a sign-in started from the prompt succeeds; the effect below then applies the star.
  const [applyAfterSignIn, setApplyAfterSignIn] = useState(0);

  /** Shows what a finished toggle means: the sign-in prompt, a failure or suspension line, or nothing. */
  const report = (result: ToggleStarResult) => {
    if (result === 'busy') return;
    const offerSignIn = result === 'signedOut' && community.available;
    setFailure(result === 'ok' || offerSignIn ? null : result === 'banned' ? 'banned' : 'failed');
    if (offerSignIn) setPrompt(true);
  };

  const toggle = () => {
    if (store) void store.toggleStar(modId).then(report);
  };

  // Signed in from the prompt: close it (state adjusted during render, not in an effect).
  if (wantStar && community.state === 'signedIn') {
    setWantStar(false);
    setPrompt(false);
    setApplyAfterSignIn((count) => count + 1);
  }

  // The prompt takes focus on its main action, the way a menu does.
  useEffect(() => {
    if (prompt) signInRef.current?.focus();
  }, [prompt]);

  useEffect(() => {
    if (applyAfterSignIn === 0) return;
    // The cached stats were anonymous; the next load asks again with the session.
    store?.invalidate();
    buttonRef.current?.focus();
    // The result is shown from the promise callback, never synchronously in the effect.
    if (store) void store.toggleStar(modId).then(report);
    // Runs once per successful sign-in; `report` reads the current account state.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [applyAfterSignIn]);

  const reachable =
    snapshot.stats.size > 0 || (community.available && community.loaded && community.state !== 'offline');
  if (!store || !snapshot.loaded || !reachable) return null;

  const current = snapshot.stats.get(modId);
  const starred = current?.starredByMe === true;
  const signingIn = community.state === 'signingIn';
  const signInError =
    community.errorCode === null
      ? null
      : community.errorCode === 'offline'
        ? t.community.offline
        : t.community.errors[community.errorCode];

  const closePrompt = () => {
    setPrompt(false);
    setWantStar(false);
    buttonRef.current?.focus();
  };

  const onClick = () => {
    if (community.available && community.state === 'signedOut') {
      setFailure(null);
      setPrompt(true);
      return;
    }
    setFailure(null);
    toggle();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    // Esc closes the prompt first; the mod page's own Esc (back to the list) waits for the next one.
    if (prompt && event.key === 'Escape' && !event.defaultPrevented) {
      event.preventDefault();
      closePrompt();
    }
  };

  return (
    <div className="star-control" onKeyDown={onKeyDown}>
      <div className="star-row">
        <button
          ref={buttonRef}
          type="button"
          className="btn star-button"
          aria-pressed={starred}
          aria-describedby={countId}
          onClick={onClick}
        >
          <Icon name="star" className="icon star-icon" />
          {t.stars.star}
        </button>
        <StarCount stars={current?.stars ?? 0} id={countId} icon={false} />
      </div>
      <p role="status" className="hint star-feedback">
        {failure === null ? '' : t.stars[failure]}
      </p>
      {prompt && (
        <div role="group" aria-labelledby={promptId} className="star-prompt">
          <p id={promptId} className="star-prompt-text">
            {t.stars.signInPrompt}
          </p>
          <div className="star-prompt-actions">
            <button
              ref={signInRef}
              type="button"
              className={`btn btn-sm btn-accent-outline${signingIn ? ' btn-busy' : ''}`}
              disabled={signingIn}
              aria-busy={signingIn ? 'true' : undefined}
              onClick={() => {
                setWantStar(true);
                community.signIn();
              }}
            >
              <Icon name="account" />
              {t.community.signIn}
            </button>
            <button type="button" className="btn btn-sm btn-ghost" onClick={closePrompt}>
              {t.stars.notNow}
            </button>
          </div>
          <p role="status" className="hint">
            {signingIn ? t.community.waiting : (signInError ?? '')}
          </p>
        </div>
      )}
    </div>
  );
}
