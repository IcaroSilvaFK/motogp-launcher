import { useState } from 'react';
import { useCommunityAccount } from '../application';
import type { CommunityProfile } from '../domain';
import { useCommunity } from './CommunityContext';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { stagger } from './motion';

/** Up to two initials from the first two words of a display name (code points, so emoji stay whole). */
export function initialsOf(displayName: string): string {
  const words = displayName.trim().split(/\s+/).filter(Boolean).slice(0, 2);
  const initials = words.map((word) => (Array.from(word)[0] ?? '').toLocaleUpperCase()).join('');
  return initials === '' ? '?' : initials;
}

/**
 * Settings > Community (#122): the Discord account row. Renders nothing without a community port.
 * The backend runs the browser sign-in and keeps the token; this row only shows the status.
 */
export function CommunityAccountSection() {
  const { t } = useI18n();
  const { account } = useCommunity();
  const community = useCommunityAccount(account);
  // A sign-in error stays until the user dismisses it or tries again.
  const [dismissed, setDismissed] = useState(false);

  if (!community.available) return null;

  const { loaded, state, profile, errorCode } = community;
  const signingIn = state === 'signingIn';
  const signedIn = state === 'signedIn' && profile !== null;
  const offline = state === 'offline';
  const error = errorCode !== null && errorCode !== 'offline' && !dismissed ? errorCode : null;

  const signIn = () => {
    setDismissed(false);
    community.signIn();
  };

  let hint = t.community.signedOutHint;
  if (signingIn) hint = t.community.waitingHint;
  else if (signedIn) hint = t.community.signedInHint;

  return (
    <section
      id="settings-community"
      aria-labelledby="settings-community-title"
      className="settings-section stagger-item"
      style={stagger(2)}
    >
      <h3 id="settings-community-title" className="group-title">
        {t.community.title}
      </h3>
      <div className="card rows" aria-busy={loaded ? undefined : 'true'}>
        {!loaded ? (
          // Holds the row's place until the first status answer, so no wrong state flashes.
          <div className="setting-row">
            <div className="account-identity">
              <span className="skeleton account-avatar" aria-hidden="true" />
              <span className="skeleton skeleton-line" aria-hidden="true" />
            </div>
            <span className="visually-hidden">{t.community.loading}</span>
          </div>
        ) : (
          <div className="setting-row setting-row-block setting-row-stack">
            <div className="setting-row-line">
              <div className="account-identity">
                <Avatar profile={signedIn ? profile : null} />
                <div className="setting-text">
                  <p className="setting-label account-name">
                    {signedIn ? profile.displayName : t.community.accountLabel}
                  </p>
                  {!offline && <p className="hint">{hint}</p>}
                </div>
              </div>
              <div className="setting-control">
                {signedIn ? (
                  <button type="button" className="btn btn-sm" onClick={community.signOut}>
                    {t.community.signOut}
                  </button>
                ) : (
                  !offline && (
                    <button
                      type="button"
                      className={`btn btn-sm btn-accent-outline${signingIn ? ' btn-busy' : ''}`}
                      disabled={signingIn}
                      aria-busy={signingIn ? 'true' : undefined}
                      onClick={signIn}
                    >
                      <Icon name="account" />
                      {t.community.signIn}
                    </button>
                  )
                )}
              </div>
            </div>

            <div className="account-result">
              <div role="status" className="account-status">
                {signingIn && t.community.waiting}
                {offline && t.community.offline}
              </div>
              {error !== null && (
                <div role="alert" className="account-error">
                  <Icon name="info" />
                  <span className="account-error-text">{t.community.errors[error]}</span>
                  <button
                    type="button"
                    className="btn btn-sm btn-ghost btn-icon"
                    aria-label={t.community.dismiss}
                    title={t.community.dismiss}
                    onClick={() => setDismissed(true)}
                  >
                    <Icon name="close" />
                  </button>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

/** The member's avatar, their initials without one, or a generic person while signed out. Decorative. */
function Avatar({ profile }: { profile: CommunityProfile | null }) {
  if (profile === null) {
    return (
      <span className="account-avatar account-avatar-empty" aria-hidden="true">
        <Icon name="account" />
      </span>
    );
  }
  if (profile.avatarDataUrl !== null) {
    return <img className="account-avatar" src={profile.avatarDataUrl} alt="" />;
  }
  return (
    <span className="account-avatar account-avatar-initials" aria-hidden="true">
      {initialsOf(profile.displayName)}
    </span>
  );
}
