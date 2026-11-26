import { useId } from 'react';
import type { MyModsHandle } from '../application';
import type { ModVersion, MyMod, VersionStatus } from '../domain';
import { Icon } from './Icon';
import type { IconName } from './Icon';
import { formatInstantDate, formatSize, useI18n } from './i18n';
import { stagger } from './motion';

const SKELETON_ROWS = [0, 1, 2];

const STATUS_BADGE: Record<VersionStatus, { className: string; icon: IconName | null }> = {
  pending: { className: 'badge badge-warning', icon: null },
  approved: { className: 'badge badge-on', icon: 'check' },
  rejected: { className: 'badge badge-error', icon: 'close' },
};

/** Newest upload first; equal dates keep the server's order. */
function newestFirst(versions: ModVersion[]): ModVersion[] {
  const time = (v: ModVersion) => Date.parse(v.uploadedAt) || 0;
  return versions
    .map((version, index) => ({ version, index }))
    .sort((a, b) => time(b.version) - time(a.version) || b.index - a.index)
    .map((item) => item.version);
}

function VersionRow({ version }: { version: ModVersion }) {
  const { t, locale } = useI18n();
  const labelId = useId();
  const my = t.publish.myMods;
  const badge = STATUS_BADGE[version.status];
  const uploaded = Date.parse(version.uploadedAt);
  return (
    <li aria-labelledby={labelId} className="my-mod-version">
      <span id={labelId} className="my-mod-version-label num">
        {my.versionLabel(version.version)}
      </span>
      <span className="mod-meta my-mod-version-meta">
        <span className="my-mod-file" title={version.fileName}>
          {version.fileName}
        </span>
        <span className="dot" aria-hidden="true" />
        <span className="num">{formatSize(version.size, locale)}</span>
        {!Number.isNaN(uploaded) && (
          <>
            <span className="dot" aria-hidden="true" />
            <span>{my.uploaded(formatInstantDate(uploaded, locale))}</span>
          </>
        )}
      </span>
      <span className={`${badge.className} my-mod-status`}>
        {badge.icon !== null && <Icon name={badge.icon} />}
        {my.status[version.status]}
      </span>
      {version.status === 'rejected' && version.rejectionReason !== undefined && (
        <p className="my-mod-reason">{my.reason(version.rejectionReason)}</p>
      )}
    </li>
  );
}

function ModCard({ mod, index }: { mod: MyMod; index: number }) {
  const { t } = useI18n();
  const titleId = useId();
  const versions = newestFirst(mod.versions);
  const latest = versions[0];
  return (
    <li className="stagger-item" style={stagger(index)}>
      <article aria-labelledby={titleId} className="card my-mod">
        <header className="my-mod-head">
          <div className="my-mod-heading">
            <h3 id={titleId} className="my-mod-title">
              {latest?.title ?? mod.id}
            </h3>
            <span className="mod-meta">
              <code className="my-mod-id">{mod.id}</code>
              {latest !== undefined && (
                <>
                  <span className="dot" aria-hidden="true" />
                  <span>{latest.category}</span>
                  <span className="dot" aria-hidden="true" />
                  <span>{latest.game}</span>
                </>
              )}
              <span className="dot" aria-hidden="true" />
              <span className="num">{t.publish.myMods.versions(versions.length)}</span>
            </span>
          </div>
        </header>
        <ul className="plain my-mod-versions">
          {versions.map((version) => (
            <VersionRow key={version.version} version={version} />
          ))}
        </ul>
      </article>
    </li>
  );
}

/**
 * "My mods" (#130): each mod the creator published, as a card with its versions newest first and a
 * review badge per version (a rejected one also shows the moderator's reason). Designed loading,
 * empty and error states; Refresh re-reads and keeps the list visible meanwhile.
 */
export function MyModsList({ myMods, onPublish }: { myMods: MyModsHandle; onPublish: () => void }) {
  const { t } = useI18n();
  const my = t.publish.myMods;
  const loading = myMods.status === 'loading';
  const busy = loading || myMods.refreshing;

  let body;
  if (loading) {
    body = (
      <>
        <p role="status" className="visually-hidden">
          {my.loading}
        </p>
        <div className="skeleton-list" aria-hidden="true">
          {SKELETON_ROWS.map((row) => (
            <div key={row} className="skeleton-row">
              <span className="skeleton skeleton-line" />
            </div>
          ))}
        </div>
      </>
    );
  } else if (myMods.status === 'error' && myMods.error !== null) {
    body = (
      <div role="alert" className="empty-state">
        <span className="empty-icon" aria-hidden="true">
          <Icon name="info" />
        </span>
        <p className="empty-title">{t.publish.errors[myMods.error.code]}</p>
        <button type="button" className="btn btn-accent-outline" disabled={busy} onClick={myMods.refresh}>
          <Icon name="refresh" />
          {my.retry}
        </button>
      </div>
    );
  } else if (myMods.mods.length === 0) {
    body = (
      <div className="empty-state">
        <span className="empty-icon" aria-hidden="true">
          <Icon name="upload" />
        </span>
        <p className="empty-title">{my.emptyTitle}</p>
        <p>{my.emptyHint}</p>
        <button type="button" className="btn btn-accent-outline" onClick={onPublish}>
          <Icon name="upload" />
          {t.publish.open}
        </button>
      </div>
    );
  } else {
    body = (
      <ul className="plain my-mods">
        {myMods.mods.map((mod, index) => (
          <ModCard key={mod.id} mod={mod} index={index} />
        ))}
      </ul>
    );
  }

  return (
    <div className="my-mods-panel" aria-busy={busy || undefined}>
      <div className="publish-toolbar">
        <button type="button" className="btn btn-sm" disabled={busy} onClick={myMods.refresh}>
          <Icon name="refresh" />
          {myMods.refreshing ? my.refreshing : my.refresh}
        </button>
      </div>
      {body}
    </div>
  );
}
