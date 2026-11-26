import { useEffect, useId } from 'react';
import { selectSessionClean, useAd } from '../application';
import type { AdProvider } from '../application';
import type { AdSlotId, AdViewModel } from '../domain';
import { useAdsEnabled } from './AdPreferenceContext';
import { useAds } from './AdsContext';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

/** Sponsor labels that only repeat the "Sponsored" disclosure (any shipped language, #123). */
const GENERIC_SPONSOR_LABELS = ['sponsored', 'patrocinado', 'house ad', 'anúncio da casa', 'anúncio'];

/** The sponsor's own name for the byline, unless it only repeats "Sponsored" in some way. */
function sponsorOf(ad: AdViewModel, label: string): string | null {
  const sponsor = ad.sponsorLabel.trim();
  const lower = sponsor.toLowerCase();
  if (sponsor === '' || lower === label.toLowerCase() || GENERIC_SPONSOR_LABELS.includes(lower)) return null;
  return sponsor;
}

function AdCard({ slot, ad, onClick }: { slot: AdSlotId; ad: AdViewModel; onClick: () => void }) {
  const { t } = useI18n();
  const id = useId();
  const labelId = `${id}-label`;
  const titleId = `${id}-title`;
  const opensId = `${id}-opens`;
  const sponsor = sponsorOf(ad, t.ads.sponsored);
  // Settings > About never shows an image: it is a support row, so the icon tile stands in.
  const image = slot === 'about' ? null : ad.imageDataUri;
  const classes = [
    // About is one more row of the About card (#136): the settings row pattern, as a support card.
    slot === 'about' ? 'setting-row' : null,
    'ad-slot',
    `ad-slot-${slot}`,
    image === null ? 'ad-text-first' : null,
  ];

  return (
    <section aria-labelledby={labelId} className={classes.filter(Boolean).join(' ')} data-slot={slot}>
      {image === null ? (
        <span className="ad-tile" aria-hidden="true">
          <Icon name="megaphone" />
        </span>
      ) : (
        <img className="ad-media" src={image} alt="" draggable={false} />
      )}
      <div className="ad-content">
        {/* The disclosure first, on its own line; the sponsor's name is a quieter byline beside
            it, never a second equal label. */}
        <div className="ad-meta">
          <p id={labelId} className="ad-label">
            {t.ads.sponsored}
          </p>
          {sponsor !== null && (
            <>
              <span className="dot" aria-hidden="true" />
              <p className="ad-byline">{sponsor}</p>
            </>
          )}
        </div>
        <div className="ad-copy">
          <p id={titleId} className="ad-title">
            {ad.title}
          </p>
          {ad.body !== '' && <p className="ad-text">{ad.body}</p>}
        </div>
      </div>
      <div className="ad-actions">
        {/* One outlined (secondary) action in every slot: the accent stays reserved for Play. */}
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
      </div>
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
 * region ("Sponsored") with the creative image (`alt=""`) or, without one, an icon tile (#123), the
 * label and the sponsor's byline, the title, body and one action (#136: one markup, laid out per
 * slot in CSS: a compact card in the sidebar, a slim banner in Discover, a row in About); the click
 * sends only the slot and the creative id, and the creative is reported as shown once per mount.
 */
export function AdSlot({ slot }: { slot: AdSlotId }) {
  const { provider } = useAds();
  // The user's sponsored-content choice (#105): off, a failing store or no store means no ads.
  const enabled = useAdsEnabled();
  if (provider === undefined || !enabled) return null;
  return <ConnectedAdSlot slot={slot} provider={provider} />;
}
