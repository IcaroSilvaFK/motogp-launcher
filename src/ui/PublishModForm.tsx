import { useEffect, useId, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import type { PublishFilePicker, PublishModHandle, PublishState } from '../application';
import { MAX_SCREENSHOTS, suggestSlug } from '../domain';
import type { PublishField, PublishForm } from '../domain';
import { BannerIcon } from './BannerIcon';
import { Icon } from './Icon';
import { formatSize, useI18n } from './i18n';

/** Visual order of the fields: the first invalid one in this order takes focus after a refused submit. */
const FIELD_ORDER: PublishField[] = [
  'title',
  'modId',
  'description',
  'longDescription',
  'category',
  'game',
  'version',
  'file',
  'screenshots',
  'ownershipConfirmed',
];

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

function emptyForm(games: string[]): PublishForm {
  return {
    modId: '',
    title: '',
    description: '',
    longDescription: '',
    category: '',
    game: games[0] ?? '',
    version: '',
    file: null,
    screenshots: [],
    ownershipConfirmed: false,
  };
}

/** Supporting text below a control (Material): the error replaces the hint while there is one. */
function Support({ id, hint, error }: { id: string; hint?: string; error?: string }) {
  if (error !== undefined) {
    return (
      <p id={id} className="publish-error">
        <Icon name="info" />
        <span>{error}</span>
      </p>
    );
  }
  if (hint === undefined) return null;
  return (
    <p id={id} className="hint publish-hint">
      {hint}
    </p>
  );
}

/** A labelled text field (input or textarea) with its hint or error wired as the description. */
function TextField({
  label,
  hint,
  error,
  value,
  onChange,
  field,
  multiline = false,
  rows,
  list,
  wide = false,
}: {
  label: string;
  hint?: string;
  error?: string;
  value: string;
  onChange: (value: string) => void;
  field: PublishField;
  multiline?: boolean;
  rows?: number;
  list?: string;
  wide?: boolean;
}) {
  const id = useId();
  const supportId = `${id}-support`;
  const described = error !== undefined || hint !== undefined ? supportId : undefined;
  const common = {
    id,
    value,
    className: multiline ? 'input publish-textarea' : 'input',
    'aria-invalid': error !== undefined ? ('true' as const) : undefined,
    'aria-describedby': described,
    'data-field': field,
  };
  return (
    <div className={`publish-field${wide ? ' publish-field-wide' : ''}`}>
      <label htmlFor={id} className="publish-label">
        {label}
      </label>
      {multiline ? (
        <textarea {...common} rows={rows} onChange={(event) => onChange(event.target.value)} />
      ) : (
        <input
          {...common}

          type="text"
          list={list}
          spellCheck={false}
          onChange={(event) => onChange(event.target.value)}
        />
      )}
      <Support id={supportId} hint={hint} error={error} />
    </div>
  );
}

/** Upload feedback: a phase label, Cancel, and a linear bar (indeterminate until bytes are known). */
function UploadProgress({ state, onCancel }: { state: PublishState; onCancel: () => void }) {
  const { t, locale } = useI18n();
  if (state.status !== 'publishing') return null;
  const progress = state.progress;
  let label = t.publish.preparing;
  let percent: number | null = null;
  if (progress !== null) {
    const known = progress.total > 0;
    if (known && (progress.phase === 'hashing' || progress.phase === 'uploading')) {
      percent = Math.min(100, Math.max(0, Math.round((progress.sent / progress.total) * 100)));
    }
    if (progress.phase === 'hashing') label = t.publish.hashing;
    else if (progress.phase === 'uploading') {
      label = t.publish.uploading(formatSize(progress.sent, locale), formatSize(progress.total, locale));
    } else label = t.publish.finishing;
  }
  return (
    <div className="publish-progress" aria-busy="true">
      <div className="install-progress-line">
        <span className="install-progress-label num">{label}</span>
        <button type="button" className="btn btn-ghost btn-sm" onClick={onCancel}>
          {t.publish.cancel}
        </button>
      </div>
      <div
        className="install-progress"
        role="progressbar"
        aria-label={t.publish.progressLabel}
        aria-valuemin={percent === null ? undefined : 0}
        aria-valuemax={percent === null ? undefined : 100}
        aria-valuenow={percent ?? undefined}
        aria-valuetext={label}
        data-indeterminate={percent === null ? '' : undefined}
      >
        <span className="install-progress-fill" style={percent === null ? undefined : { width: `${percent}%` }} />
      </div>
    </div>
  );
}

/** A titled group of fields inside the form card. */
function Section({ title, children }: { title: string; children: ReactNode }) {
  const id = useId();
  return (
    <section aria-labelledby={id} className="publish-section">
      <h3 id={id} className="section-title">
        {title}
      </h3>
      <div className="publish-grid">{children}</div>
    </section>
  );
}

/**
 * The publish form (#130): details, files and the ownership confirmation in one card, the one
 * primary action (Publish) at its foot. The mod id follows the title (`suggestSlug`) until the
 * creator edits it. A refused submit shows each field's error under it and focuses the first one;
 * editing a field hides its stale error. While uploading, the fields lock and the foot shows the
 * progress with Cancel; a failure keeps the form and offers Try again; success shows the result.
 */
export function PublishModForm({
  publish,
  picker,
  categories,
  games,
  onPublished,
  onViewMyMods,
}: {
  publish: PublishModHandle;
  picker: PublishFilePicker | undefined;
  categories: string[];
  games: string[];
  onPublished: () => void;
  onViewMyMods: () => void;
}) {
  const { t } = useI18n();
  const p = t.publish;
  const [form, setForm] = useState<PublishForm>(() => emptyForm(games));
  const [modIdEdited, setModIdEdited] = useState(false);
  // Fields changed since the last refused submit: their old error no longer applies.
  const [edited, setEdited] = useState<ReadonlySet<PublishField>>(new Set());
  const [attempt, setAttempt] = useState(0);
  const formRef = useRef<HTMLFormElement>(null);
  const doneRef = useRef<HTMLHeadingElement>(null);
  const addShotsRef = useRef<HTMLButtonElement>(null);
  const listsId = useId();
  const ownershipErrorId = useId();
  const fileSupportId = useId();
  const shotsSupportId = useId();
  const footerStatusId = useId();
  const { state, busy } = publish;

  const errorOf = (field: PublishField): string | undefined => {
    const key = publish.errors[field];
    return key === undefined || edited.has(field) ? undefined : p.validation[key];
  };
  const visibleErrors = FIELD_ORDER.filter((field) => errorOf(field) !== undefined).length;

  // After a refused submit, focus the first invalid field (visual order).
  useEffect(() => {
    if (attempt === 0) return;
    const first = FIELD_ORDER.find((field) => publish.errors[field] !== undefined);
    if (first !== undefined) formRef.current?.querySelector<HTMLElement>(`[data-field="${first}"]`)?.focus();
  }, [attempt, publish.errors]);

  // Success replaces the form: move focus to its heading so it is announced.
  useEffect(() => {
    if (state.status === 'done') doneRef.current?.focus();
  }, [state.status]);

  const change = <K extends PublishField>(field: K, value: PublishForm[K]) => {
    setForm((current) => ({ ...current, [field]: value }));
    setEdited((current) => (current.has(field) ? current : new Set(current).add(field)));
  };

  const changeTitle = (title: string) => {
    change('title', title);
    if (!modIdEdited) change('modId', suggestSlug(title));
  };

  const changeModId = (modId: string) => {
    change('modId', modId);
    // Clearing the id hands it back to the title.
    setModIdEdited(modId !== '');
  };

  const submit = async () => {
    if (busy) return;
    setEdited(new Set());
    const published = await publish.start(form);
    if (published) onPublished();
    else setAttempt((n) => n + 1);
  };

  const pickFile = async () => {
    const path = await picker?.pickModFile().catch(() => null);
    if (path) change('file', path);
  };

  const pickScreenshots = async () => {
    const paths = await picker?.pickScreenshots().catch(() => null);
    if (!paths || paths.length === 0) return;
    const merged = [...form.screenshots, ...paths.filter((path) => !form.screenshots.includes(path))];
    change('screenshots', merged.slice(0, MAX_SCREENSHOTS));
  };

  const removeScreenshot = (path: string) => {
    change(
      'screenshots',
      form.screenshots.filter((shot) => shot !== path),
    );
    addShotsRef.current?.focus();
  };

  const publishAnother = () => {
    publish.reset();
    setForm(emptyForm(games));
    setModIdEdited(false);
    setEdited(new Set());
    setAttempt(0);
  };

  if (state.status === 'done') {
    return (
      <div className="empty-state publish-done">
        <span className="empty-icon publish-done-icon" aria-hidden="true">
          <Icon name="check" />
        </span>
        <h3 ref={doneRef} tabIndex={-1} className="empty-title">
          {p.doneTitle}
        </h3>
        <p>{p.doneBody(state.version.title, state.version.version)}</p>
        <div className="empty-actions">
          <button type="button" className="btn btn-accent-outline" onClick={onViewMyMods}>
            {p.viewMyMods}
          </button>
          <button type="button" className="btn" onClick={publishAnother}>
            <Icon name="upload" />
            {p.publishAnother}
          </button>
        </div>
      </div>
    );
  }

  const fileError = errorOf('file');
  const shotsError = errorOf('screenshots');
  const ownershipError = errorOf('ownershipConfirmed');
  const full = form.screenshots.length >= MAX_SCREENSHOTS;

  return (
    <form
      ref={formRef}
      className="card publish-form"
      noValidate
      aria-busy={busy || undefined}
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <fieldset className="publish-fieldset" disabled={busy}>
        <Section title={p.sectionDetails}>
          <TextField label={p.title} value={form.title} error={errorOf('title')} onChange={changeTitle} field="title" />
          <TextField
            label={p.modId}
            hint={p.modIdHint}
            value={form.modId}
            error={errorOf('modId')}
            onChange={changeModId}
            field="modId"
          />
          <TextField
            label={p.description}
            hint={p.descriptionHint}
            value={form.description}
            error={errorOf('description')}
            onChange={(value) => change('description', value)}
            field="description"
            multiline
            rows={2}
            wide
          />
          <TextField
            label={p.longDescription}
            hint={p.longDescriptionHint}
            value={form.longDescription}
            error={errorOf('longDescription')}
            onChange={(value) => change('longDescription', value)}
            field="longDescription"
            multiline
            rows={6}
            wide
          />
          <TextField
            label={p.category}
            value={form.category}
            error={errorOf('category')}
            onChange={(value) => change('category', value)}
            field="category"
            list={`${listsId}-categories`}
          />
          <TextField
            label={p.game}
            value={form.game}
            error={errorOf('game')}
            onChange={(value) => change('game', value)}
            field="game"
            list={`${listsId}-games`}
          />
          <TextField
            label={p.version}
            hint={p.versionHint}
            value={form.version}
            error={errorOf('version')}
            onChange={(value) => change('version', value)}
            field="version"
          />
          <datalist id={`${listsId}-categories`}>
            {categories.map((category) => (
              <option key={category} value={category} />
            ))}
          </datalist>
          <datalist id={`${listsId}-games`}>
            {games.map((game) => (
              <option key={game} value={game} />
            ))}
          </datalist>
        </Section>

        <Section title={p.sectionFiles}>
          <div className="publish-field publish-field-wide">
            <span className="publish-label">{p.file}</span>
            <div className={`publish-file${fileError !== undefined ? ' publish-file-invalid' : ''}`}>
              <Icon name="file" />
              <span className="publish-file-name" title={form.file ?? undefined}>
                {form.file === null ? <span className="hint">{p.noFile}</span> : fileName(form.file)}
              </span>
              <button
                data-field="file"
                type="button"
                className="btn btn-sm"
                aria-describedby={fileSupportId}
                disabled={picker === undefined}
                onClick={() => void pickFile()}
              >
                {form.file === null ? p.chooseFile : p.changeFile}
              </button>
            </div>
            <Support id={fileSupportId} hint={p.fileHint} error={fileError} />
          </div>

          <div className="publish-field publish-field-wide">
            <div className="publish-label-line">
              <span className="publish-label">{p.screenshots}</span>
              <span className="hint num">{p.screenshotCount(form.screenshots.length, MAX_SCREENSHOTS)}</span>
            </div>
            {form.screenshots.length > 0 && (
              <ul className="plain publish-shots" aria-label={p.screenshots}>
                {form.screenshots.map((path) => {
                  const name = fileName(path);
                  return (
                    <li key={path} className="publish-shot">
                      <Icon name="image" />
                      <span className="publish-shot-name" title={path}>
                        {name}
                      </span>
                      <button
                        type="button"
                        className="btn btn-ghost btn-sm btn-icon"
                        aria-label={p.removeScreenshot(name)}
                        title={p.removeScreenshot(name)}
                        onClick={() => removeScreenshot(path)}
                      >
                        <Icon name="close" />
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
            <div>
              <button
                ref={addShotsRef}
                data-field="screenshots"
                type="button"
                className="btn btn-sm"
                aria-describedby={shotsSupportId}
                disabled={picker === undefined || full}
                onClick={() => void pickScreenshots()}
              >
                <Icon name="plus" />
                {p.addScreenshots}
              </button>
            </div>
            <Support id={shotsSupportId} hint={p.screenshotsHint} error={shotsError} />
          </div>
        </Section>

        <div className="publish-field publish-ownership">
          <label className="check-row publish-check">
            <input
              data-field="ownershipConfirmed"
              type="checkbox"
              checked={form.ownershipConfirmed}
              aria-invalid={ownershipError !== undefined ? 'true' : undefined}
              aria-describedby={ownershipError !== undefined ? ownershipErrorId : undefined}
              onChange={(event) => change('ownershipConfirmed', event.target.checked)}
            />
            <span>{p.ownership}</span>
          </label>
          <Support id={ownershipErrorId} error={ownershipError} />
        </div>
      </fieldset>

      {state.status === 'failed' && (
        <div role="alert" className="notice notice-error">
          <BannerIcon variant="error" />
          <p>{p.errors[state.code]}</p>
          <div className="notice-actions">
            <button type="button" className="btn btn-sm" onClick={() => void submit()}>
              <Icon name="refresh" />
              {p.retry}
            </button>
            <button type="button" className="btn btn-ghost btn-sm" onClick={publish.reset}>
              {p.dismiss}
            </button>
          </div>
        </div>
      )}
      {state.status === 'cancelled' && (
        <div role="status" className="notice notice-info">
          <BannerIcon variant="info" />
          <p>{p.cancelled}</p>
          <button type="button" className="btn btn-ghost btn-sm" onClick={publish.reset}>
            {p.dismiss}
          </button>
        </div>
      )}

      <div className="publish-footer">
        <div className="publish-footer-status">
          <UploadProgress state={state} onCancel={publish.cancel} />
          <p id={footerStatusId} role="status" className="publish-summary">
            {!busy && visibleErrors > 0 ? p.fixErrors(visibleErrors) : ''}
          </p>
        </div>
        <button
          type="submit"
          className={`btn btn-accent-outline${busy ? ' btn-busy' : ''}`}
          disabled={busy}
          aria-busy={busy ? 'true' : undefined}
        >
          <Icon name="upload" />
          {busy ? p.submitting : p.submit}
        </button>
      </div>
    </form>
  );
}
