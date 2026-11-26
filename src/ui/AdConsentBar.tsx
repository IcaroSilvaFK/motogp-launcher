import { selectSessionClean } from '../application';
import { shouldPromptForAds } from '../domain';
import { useAdPreference } from './AdPreferenceContext';
import { BannerIcon } from './BannerIcon';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

/** First-launch prompt for sponsored content. Non-modal: no dialog role, no backdrop, no focus trap. */
export function AdConsentBar() {
  const { t } = useI18n();
  const { state } = useLauncher();
  const { preference, setPreference } = useAdPreference();
  if (!shouldPromptForAds(preference) || state.status === 'loading' || !selectSessionClean(state)) return null;

  return (
    <section className="notice notice-info ad-consent" role="region" aria-label={t.ads.consentLabel}>
      <BannerIcon variant="info" />
      <p>{t.ads.consentText}</p>
      <div className="notice-actions">
        <button type="button" className="btn btn-ghost" onClick={() => setPreference('on')}>
          {t.ads.keep}
        </button>
        <button type="button" className="btn btn-ghost" onClick={() => setPreference('off')}>
          {t.ads.turnOff}
        </button>
        <button
          type="button"
          className="btn btn-ghost btn-icon"
          aria-label={t.dismiss}
          onClick={() => setPreference('on')}
        >
          <Icon name="close" />
        </button>
      </div>
    </section>
  );
}
