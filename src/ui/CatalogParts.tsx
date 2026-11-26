import type { CommandError } from '../domain';
import { BannerIcon } from './BannerIcon';
import { Icon } from './Icon';
import { useI18n } from './i18n';

/** Inline error row (refresh or Open page failure) with a Dismiss action. */
export function InlineError({ error, onDismiss }: { error: CommandError; onDismiss: () => void }) {
  const { t } = useI18n();
  return (
    <div role="alert" className="notice notice-error">
      <BannerIcon variant="error" />
      <p>{t.errors[error.code]}</p>
      <button type="button" className="btn btn-ghost" onClick={onDismiss}>
        {t.dismiss}
      </button>
    </div>
  );
}

/** A neutral category cover, used where an entry has no thumbnail (or it failed to load). */
export function CategoryCover({ category, className = 'catalog-cover' }: { category: string; className?: string }) {
  return (
    <span className={className} aria-hidden="true">
      <Icon name="library" className="catalog-cover-icon" />
      <span className="catalog-cover-label">{category}</span>
    </span>
  );
}
