import { useEffect, useId } from 'react';
import { selectSessionClean, useAd } from '../application';
import type { AdProvider } from '../application';
import type { AdSlotId, AdViewModel } from '../domain';
import { useAdsEnabled } from './AdPreferenceContext';
import { useAds } from './AdsContext';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

/** The sponsor's own label, unless it only repeats "Sponsored" (in any language shown). */
function sponsorOf(ad: AdViewModel, label: string): string | null {
  const sponsor = ad.sponsorLabel.trim();
  const lower = sponsor.toLowerCase();
  if (sponsor === '' || lower === label.toLowerCase() || lower === 'sponsored') return null;
  return sponsor;
}

function AdCard({ slot, ad, onClick }: { slot: AdSlotId; ad: AdViewModel; onClick: () => void }) {
  const { t } = useI18n();
  const id = useId();
  const labelId = `${id}-label`;
  const titleId = `${id}-title`;
  const opensId = `${id}-opens`;
  const sponsor = sponsorOf(ad, t.ads.sponsored);

  const label = (
    <p className="ad-label">
      <span id={labelId}>{t.ads.sponsored}</span>
      {sponsor !== null && (
        <>
          <span className="dot" aria-hidden="true" />
          <span className="ad-sponsor">{sponsor}</span>
        </>
      )}
    </p>
  );

  const action = (
    <>
      <button
        type="button"
        className="btn btn-sm ad-action"
        aria-describedby={`${titleId} ${opensId}`}
        onClick={onClick}
      >
        <Icon name="external" />
        {t.ads.learnMore}
      </button>
      <span id={opensId} hidden>
        {t.ads.opensInBrowser}
      </span>
    </>
  );

  // Settings > About: a quiet setting row, no image (the least visible slot).
  if (slot === 'about') {
    return (
      <section aria-labelledby={labelId} className="setting-row ad-slot ad-slot-about" data-slot={slot}>
        <div className="setting-text">
          {label}
          <p className="hint ad-line">
            <span id={titleId} className="ad-title">
              {ad.title}
            </span>
            {ad.body !== '' && (
              <>
                <span className="dot" aria-hidden="true" />
                <span className="ad-text">{ad.body}</span>
              </>
            )}
          </p>
        </div>
        <div className="setting-control">{action}</div>
      </section>
    );
  }

  return (
    <section aria-labelledby={labelId} className={`ad-slot ad-slot-${slot}`} data-slot={slot}>
      <img className="ad-image" src={ad.imageDataUri} alt="" draggable={false} />
      <div className="ad-content">
        {label}
        <p id={titleId} className="ad-title">
          {ad.title}
        </p>
        {ad.body !== '' && <p className="ad-text">{ad.body}</p>}
      </div>
      <div className="ad-actions">{action}</div>
    </section>
  );
}

function ConnectedAdSlot({ slot, provider }: { slot: AdSlotId; provider: AdProvider }) {
  const { state } = useLauncher();
  // Never during skeleton loading or any active (or unknown) session: starting, running, stopping, cleaning up.
  const allowed = state.status !== 'loading' && selectSessionClean(state);
  const { ad, click, shown } = useAd(provider, slot, allowed);

  useEffect(() => {
    if (ad !== null) shown();
  }, [ad, shown]);

  if (ad === null) return null;
  return <AdCard slot={slot} ad={ad} onClick={click} />;
}

/**
 * A sponsored placement (#104). Renders nothing (no placeholder, no spinner) until a creative has
 * loaded, when ads are off, without a provider, while the launcher loads and during a game
 * session; the fetch runs in an effect, so it never delays Play or a screen. A card is a labeled
 * region ("Sponsored") with the creative image (`alt=""`), title, body and one action; the click
 * sends only the slot and the creative id, and the creative is reported as shown once per mount.
 */
export function AdSlot({ slot }: { slot: AdSlotId }) {
  const { provider } = useAds();
  // The user's sponsored-content choice (#105): off, a failing store or no store means no ads.
  const enabled = useAdsEnabled();
  if (provider === undefined || !enabled) return null;
  return <ConnectedAdSlot slot={slot} provider={provider} />;
}
