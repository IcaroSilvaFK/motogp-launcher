import { useCallback, useEffect, useRef, useState } from 'react';
import type { CommandError, StorageUsage } from '../domain';
import { storageBreakdown, toCommandError } from '../domain';
import { Icon } from './Icon';
import { formatShare, formatSize, useI18n } from './i18n';
import { useLauncher } from './LauncherContext';
import { stagger } from './motion';

/**
 * Settings > Storage: where the launcher keeps its data and how much it uses. Self-contained on
 * purpose (local state only): measuring never blocks the rest of Settings and never raises a
 * global notice; failures are shown inline with Retry.
 */
export function StorageSection() {
  const { t, locale } = useI18n();
  const { controller } = useLauncher();
  const [usage, setUsage] = useState<StorageUsage | null>(null);
  const [measuring, setMeasuring] = useState(true);
  const [error, setError] = useState<CommandError | null>(null);
  const [openFailed, setOpenFailed] = useState(false);
  /** Id of the latest request: an older answer (or one after unmount) is ignored. */
  const latest = useRef(0);

  const request = useCallback(() => {
    latest.current += 1;
    const id = latest.current;
    controller.getStorageUsage().then(
      (next) => {
        if (id !== latest.current) return;
        setUsage(next);
        setError(null);
        setMeasuring(false);
      },
      (caught: unknown) => {
        if (id !== latest.current) return;
        setError(toCommandError(caught));
        setMeasuring(false);
      },
    );
  }, [controller]);

  useEffect(() => {
    request();
    return () => {
      latest.current += 1;
    };
  }, [request]);

  const refresh = () => {
    setMeasuring(true);
    request();
  };

  const openFolder = () => {
    setOpenFailed(false);
    controller.openDataFolder().catch(() => setOpenFailed(true));
  };

  const firstLoad = usage === null && measuring;
  const skeleton = <span className="skeleton skeleton-line storage-skeleton" />;
  const empty = '—';
  const total = usage === null ? null : formatSize(usage.totalBytes, locale);
  const free =
    usage === null ? null : usage.freeBytes === null ? t.storage.unavailable : formatSize(usage.freeBytes, locale);

  return (
    <section
      id="settings-storage"
      aria-labelledby="settings-storage-title"
      className="settings-section stagger-item"
      style={stagger(2)}
    >
      <h3 id="settings-storage-title" className="group-title">
        {t.storage.title}
      </h3>
      <div className="card rows storage-card" aria-busy={measuring}>
        <span className="visually-hidden" role="status">
          {measuring ? t.storage.measuring : ''}
        </span>

        {error !== null && (
          <div className="setting-row storage-error" role="alert">
            <div className="setting-text">
              <p className="setting-label">{t.storage.measureFailed}</p>
              <p className="hint">{t.errors[error.code]}</p>
            </div>
            <div className="setting-control">
              <button type="button" className="btn btn-sm" onClick={refresh} disabled={measuring}>
                <Icon name="refresh" />
                {t.storage.retry}
              </button>
            </div>
          </div>
        )}

        <div className="setting-row">
          <div className="setting-text">
            <p className="setting-label">{t.storage.folderLabel}</p>
            {usage !== null ? (
              <code className="storage-path" title={usage.root}>
                {usage.root}
              </code>
            ) : firstLoad ? (
              skeleton
            ) : (
              <p className="hint">{t.storage.folderHint}</p>
            )}
            {openFailed && (
              <p className="storage-inline-error" role="alert">
                {t.storage.openFailed}
              </p>
            )}
          </div>
          <div className="setting-control">
            <button type="button" className="btn btn-sm" onClick={openFolder}>
              <Icon name="folder" />
              {t.storage.openFolder}
            </button>
          </div>
        </div>

        <div className="setting-row">
          <dl className="storage-stats">
            <div className="storage-stat">
              <dt>{t.storage.totalUsed}</dt>
              <dd className="num">{total ?? (firstLoad ? skeleton : empty)}</dd>
            </div>
            <div className="storage-stat">
              <dt>{t.storage.freeSpace}</dt>
              <dd className="num">{free ?? (firstLoad ? skeleton : empty)}</dd>
            </div>
          </dl>
          <div className="setting-control">
            <button type="button" className="btn btn-sm" onClick={refresh} disabled={measuring}>
              <Icon name="refresh" />
              {t.storage.refresh}
            </button>
          </div>
        </div>

        {(usage !== null || firstLoad) && (
          <div className="setting-row setting-row-block">
            <ul className="plain storage-breakdown" aria-label={t.storage.breakdownLabel}>
              {usage === null
                ? [0, 1, 2].map((key) => (
                    <li key={key} className="storage-category" aria-hidden="true">
                      {skeleton}
                    </li>
                  ))
                : storageBreakdown(usage).map(({ category, bytes, share }, index) => {
                    const name = t.storage.categories[category];
                    const size = formatSize(bytes, locale);
                    const percent = formatShare(share, locale);
                    const value = Math.round(share * 1000) / 10;
                    return (
                      <li key={category} className="storage-category stagger-item" style={stagger(index)}>
                        <div className="storage-category-text">
                          <span className="storage-category-name">{name}</span>
                          <span className="hint">{t.storage.categoryHints[category]}</span>
                        </div>
                        <div
                          role="meter"
                          className="storage-meter"
                          aria-label={name}
                          aria-valuemin={0}
                          aria-valuemax={100}
                          aria-valuenow={value}
                          aria-valuetext={`${size}, ${percent}`}
                        >
                          <span
                            className="storage-meter-fill"
                            data-filled={bytes > 0 ? '' : undefined}
                            style={{ width: `${value}%` }}
                          />
                        </div>
                        <span className="num storage-size">{size}</span>
                        <span className="num storage-share">{percent}</span>
                      </li>
                    );
                  })}
            </ul>
          </div>
        )}
      </div>
    </section>
  );
}
