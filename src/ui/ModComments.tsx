import { useEffect, useId, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { useCommunityAccount, useModComments } from '../application';
import { commentDraftProblem, MAX_COMMENT_LENGTH } from '../domain';
import type { ModComment } from '../domain';
import { initialsOf } from './CommunityAccountSection';
import { useCommunity } from './CommunityContext';
import { Icon } from './Icon';
import { useI18n } from './i18n';
import type { Locale } from './i18n';
import { SafeMarkdown } from './SafeMarkdown';

/** Largest unit first; a month is 30 days and a year 365, which is enough for "how long ago". */
const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 365 * 86400],
  ['month', 30 * 86400],
  ['week', 7 * 86400],
  ['day', 86400],
  ['hour', 3600],
  ['minute', 60],
];

/**
 * "3 hours ago", "yesterday", "now" in the given locale: the largest whole unit that fits. A time
 * ahead of `now` (the server clock ahead of this PC) reads as "now".
 */
export function formatRelativeTime(ms: number, locale: Locale, now: number = Date.now()): string {
  const seconds = Math.min(0, Math.round((ms - now) / 1000));
  const format = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' });
  for (const [unit, size] of UNITS) {
    if (-seconds >= size) return format.format(Math.trunc(seconds / size), unit);
  }
  return format.format(0, 'second');
}

/** Characters as the server counts them: code points after trimming. */
function draftLength(text: string): number {
  return [...text.replace(/\r\n/g, '\n').trim()].length;
}

/**
 * Mod page > Comments (#118), under the description. Anyone can read; posting, editing and deleting
 * need the Discord sign-in. Renders nothing without a comments port. Offline is a quiet status so the
 * rest of the page keeps working; write failures show one dismissible alert above the composer.
 */
export function ModComments({ modId }: { modId: string }) {
  const { t } = useI18n();
  const { account, comments: port } = useCommunity();
  const comments = useModComments(port, modId);
  const community = useCommunityAccount(account);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const text = t.comments;

  if (!comments.available) return null;

  const { status, error } = comments;
  // The server forgot the session: offer the sign-in again even if the account row still says signed in.
  const sessionLost = error?.code === 'signed_out';

  const signIn = () => {
    comments.clearError();
    community.signIn();
  };

  let footer = null;
  if (status === 'ready' && community.available && community.loaded) {
    if (community.state === 'signedIn' && !sessionLost) {
      footer = (
        <DraftForm
          className="comment-composer"
          label={text.composerLabel}
          placeholder={text.placeholder}
          submitLabel={text.post}
          busy={comments.busy}
          clearOnSuccess
          onSubmit={comments.post}
        />
      );
    } else if (community.state === 'offline') {
      footer = <p className="hint comment-invite">{text.accountOffline}</p>;
    } else {
      const signingIn = community.state === 'signingIn';
      const { errorCode } = community;
      footer = (
        <div className="comment-invite">
          <button
            type="button"
            className={`btn btn-sm btn-accent-outline${signingIn ? ' btn-busy' : ''}`}
            disabled={signingIn}
            aria-busy={signingIn ? 'true' : undefined}
            onClick={signIn}
          >
            <Icon name="account" />
            {text.signIn}
          </button>
          <p className="hint">{text.signInHint}</p>
          {errorCode !== null && errorCode !== 'offline' && (
            <p className="comment-error-text">{t.community.errors[errorCode]}</p>
          )}
        </div>
      );
    }
  }

  return (
    <section aria-label={text.title} className="mod-section mod-comments">
      <h3 ref={headingRef} tabIndex={-1} className="group-title mod-comments-title">
        {text.title}
        {status === 'ready' && (
          <>
            {' '}
            <span className="mod-comments-count num">{text.count(comments.comments.length, comments.hasMore)}</span>
          </>
        )}
      </h3>

      {status === 'loading' && (
        <div className="comment-list" aria-busy="true">
          {[0, 1, 2].map((key) => (
            <div key={key} className="comment comment-skeleton" aria-hidden="true">
              <span className="skeleton comment-avatar" />
              <div className="comment-main">
                <span className="skeleton skeleton-line comment-skeleton-meta" />
                <span className="skeleton skeleton-line" />
              </div>
            </div>
          ))}
          <span className="visually-hidden">{text.loading}</span>
        </div>
      )}

      {status === 'offline' && (
        <div className="comments-offline">
          <Icon name="info" />
          <p role="status" className="comments-offline-text">
            {text.offline}
          </p>
          <button type="button" className="btn btn-sm btn-ghost" onClick={comments.reload}>
            <Icon name="refresh" />
            {text.retry}
          </button>
        </div>
      )}

      {status === 'ready' && comments.comments.length === 0 && (
        <div className="empty-state empty-state-compact">
          <span className="empty-icon" aria-hidden="true">
            <Icon name="comment" />
          </span>
          <p className="empty-title">{text.emptyTitle}</p>
          <p>{text.emptyHint}</p>
        </div>
      )}

      {status === 'ready' && comments.comments.length > 0 && (
        <ol className="comment-list">
          {comments.comments.map((comment) => (
            <CommentItem
              key={comment.id}
              comment={comment}
              busy={comments.busy}
              onEdit={comments.edit}
              onRemove={comments.remove}
              onRemoved={() => headingRef.current?.focus()}
            />
          ))}
        </ol>
      )}

      {status === 'ready' && comments.hasMore && (
        <button
          type="button"
          className={`btn btn-sm comment-more${comments.loadingMore ? ' btn-busy' : ''}`}
          disabled={comments.loadingMore}
          aria-busy={comments.loadingMore ? 'true' : undefined}
          onClick={comments.loadMore}
        >
          {text.loadMore}
        </button>
      )}

      {error !== null && status === 'ready' && (
        <div role="alert" className="comment-alert">
          <Icon name="info" />
          <span className="comment-error-text">
            {error.code === 'rate_limited'
              ? text.errors.rate_limited(error.retryAfterSecs ?? 1)
              : text.errors[error.code]}
          </span>
          <button
            type="button"
            className="btn btn-sm btn-ghost btn-icon"
            aria-label={text.dismiss}
            title={text.dismiss}
            onClick={comments.clearError}
          >
            <Icon name="close" />
          </button>
        </div>
      )}

      {footer}
    </section>
  );
}

/** One comment: avatar, name, relative date, body; Edit and Delete (with a confirm step) on own ones. */
function CommentItem({
  comment,
  busy,
  onEdit,
  onRemove,
  onRemoved,
}: {
  comment: ModComment;
  busy: boolean;
  onEdit: (commentId: string, body: string) => Promise<boolean>;
  onRemove: (commentId: string) => Promise<boolean>;
  onRemoved: () => void;
}) {
  const { t, locale } = useI18n();
  const text = t.comments;
  const [mode, setMode] = useState<'view' | 'edit' | 'confirm'>('view');
  const editRef = useRef<HTMLButtonElement>(null);
  const deleteRef = useRef<HTMLButtonElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  // Where focus returns when the editor or the confirmation closes.
  const returnTo = useRef<'edit' | 'delete' | null>(null);
  const confirmId = useId();

  useEffect(() => {
    if (mode === 'confirm') cancelRef.current?.focus();
    if (mode === 'view' && returnTo.current !== null) {
      (returnTo.current === 'edit' ? editRef : deleteRef).current?.focus();
      returnTo.current = null;
    }
  }, [mode]);

  const close = (from: 'edit' | 'delete') => {
    returnTo.current = from;
    setMode('view');
  };

  const confirmDelete = () => {
    void onRemove(comment.id).then((removed) => {
      if (removed) onRemoved();
      else close('delete');
    });
  };

  const onConfirmKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    close('delete');
  };

  const created = new Date(comment.createdAtMs);
  const absolute = new Intl.DateTimeFormat(locale, { dateStyle: 'long', timeStyle: 'short' }).format(created);

  return (
    <li className={`comment${comment.own ? ' comment-own' : ''}`}>
      {comment.authorAvatarDataUrl !== null ? (
        <img className="comment-avatar" src={comment.authorAvatarDataUrl} alt="" />
      ) : (
        <span className="comment-avatar comment-avatar-initials" aria-hidden="true">
          {initialsOf(comment.authorName)}
        </span>
      )}
      <div className="comment-main">
        <div className="comment-meta">
          <span className="comment-author">{comment.authorName}</span>
          {comment.own && <span className="chip comment-chip">{text.you}</span>}
          <time className="comment-time" dateTime={created.toISOString()} title={absolute}>
            {formatRelativeTime(comment.createdAtMs, locale)}
          </time>
          {comment.edited && <span className="comment-edited">{text.edited}</span>}
        </div>

        {mode === 'edit' ? (
          <DraftForm
            className="comment-editor"
            label={text.editLabel}
            initial={comment.body}
            submitLabel={text.save}
            busy={busy}
            autoFocus
            onSubmit={(body) => onEdit(comment.id, body)}
            onDone={() => close('edit')}
            onCancel={() => close('edit')}
            cancelLabel={text.cancel}
          />
        ) : (
          <div className="comment-body">
            <SafeMarkdown source={comment.body} />
          </div>
        )}

        {comment.own && mode === 'view' && (
          <div className="comment-actions">
            <button ref={editRef} type="button" className="btn btn-sm btn-ghost" onClick={() => setMode('edit')}>
              <Icon name="edit" />
              {text.edit}
            </button>
            <button ref={deleteRef} type="button" className="btn btn-sm btn-ghost" onClick={() => setMode('confirm')}>
              <Icon name="trash" />
              {text.delete}
            </button>
          </div>
        )}

        {mode === 'confirm' && (
          <div role="group" aria-labelledby={confirmId} className="comment-confirm" onKeyDown={onConfirmKeyDown}>
            <p id={confirmId} className="comment-confirm-text">
              {text.confirmDelete}
            </p>
            <div className="comment-actions">
              <button
                type="button"
                className={`btn btn-sm btn-danger${busy ? ' btn-busy' : ''}`}
                disabled={busy}
                aria-busy={busy ? 'true' : undefined}
                onClick={confirmDelete}
              >
                <Icon name="trash" />
                {text.delete}
              </button>
              <button ref={cancelRef} type="button" className="btn btn-sm btn-ghost" onClick={() => close('delete')}>
                {text.cancel}
              </button>
            </div>
          </div>
        )}
      </div>
    </li>
  );
}

/**
 * A comment draft: labelled textarea, live character counter against the 2000 cap, and a submit
 * that stays disabled while the draft is empty, too long, unchanged or a write is in flight.
 * Ctrl+Enter submits; Esc cancels when there is a Cancel.
 */
function DraftForm({
  className,
  label,
  placeholder,
  initial = '',
  submitLabel,
  cancelLabel,
  busy,
  autoFocus = false,
  clearOnSuccess = false,
  onSubmit,
  onDone,
  onCancel,
}: {
  className: string;
  label: string;
  placeholder?: string;
  initial?: string;
  submitLabel: string;
  cancelLabel?: string;
  busy: boolean;
  autoFocus?: boolean;
  clearOnSuccess?: boolean;
  onSubmit: (body: string) => Promise<boolean>;
  onDone?: () => void;
  onCancel?: () => void;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState(initial);
  const fieldId = useId();
  const counterId = useId();
  const overId = useId();
  const fieldRef = useRef<HTMLTextAreaElement>(null);
  const used = draftLength(draft);
  const over = Math.max(0, used - MAX_COMMENT_LENGTH);
  const unchanged = draft.trim() === initial.trim() && initial !== '';
  const blocked = busy || unchanged || commentDraftProblem(draft) !== null;

  useEffect(() => {
    if (!autoFocus) return;
    const field = fieldRef.current;
    field?.focus();
    field?.setSelectionRange(field.value.length, field.value.length);
  }, [autoFocus]);

  const submit = () => {
    if (blocked) return;
    void onSubmit(draft).then((stored) => {
      if (!stored) return;
      if (clearOnSuccess) setDraft('');
      onDone?.();
    });
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      submit();
    } else if (event.key === 'Escape' && onCancel) {
      event.preventDefault();
      onCancel();
    }
  };

  return (
    <form
      className={`comment-draft ${className}`}
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <label htmlFor={fieldId} className="visually-hidden">
        {label}
      </label>
      <textarea
        ref={fieldRef}
        id={fieldId}
        className="input comment-input"
        rows={3}
        value={draft}
        placeholder={placeholder}
        aria-describedby={`${counterId} ${overId}`}
        aria-invalid={over > 0 ? 'true' : undefined}
        onChange={(event) => setDraft(event.target.value)}
        onKeyDown={onKeyDown}
      />
      <div className="comment-draft-footer">
        <span id={counterId} className={`comment-counter num${over > 0 ? ' comment-counter-over' : ''}`}>
          {t.comments.counter(used, MAX_COMMENT_LENGTH)}
        </span>
        <span id={overId} role="status" className="comment-error-text">
          {over > 0 ? t.comments.tooLong(over) : ''}
        </span>
        <span className="comment-draft-actions">
          {!onCancel && <span className="hint comment-shortcut">{t.comments.shortcutHint}</span>}
          {onCancel && cancelLabel && (
            <button type="button" className="btn btn-sm btn-ghost" onClick={onCancel}>
              {cancelLabel}
            </button>
          )}
          <button
            type="submit"
            className={`btn btn-sm btn-accent-outline${busy ? ' btn-busy' : ''}`}
            disabled={blocked}
            aria-busy={busy ? 'true' : undefined}
          >
            {submitLabel}
          </button>
        </span>
      </div>
    </form>
  );
}
