import { useEffect, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import { createPortal } from 'react-dom';
import { Icon } from './Icon';
import { useI18n } from './i18n';

/** Shared attributes of every remote screenshot: lazy, off the main thread and without a referrer. */
const REMOTE_IMAGE = { loading: 'lazy', decoding: 'async', referrerPolicy: 'no-referrer' } as const;

/**
 * The screenshot shown as the main image. A selection that failed to load falls back to the next
 * screenshot that still works (or the first one), so the gallery never shows a broken image.
 */
function currentShot(all: string[], valid: string[], selected: string | null): string {
  const first = valid[0] ?? '';
  if (selected === null || valid.includes(selected)) return selected ?? first;
  const after = all.slice(all.indexOf(selected) + 1).find((url) => valid.includes(url));
  return after ?? first;
}

/** Full-screen viewer: a modal dialog with the image, a counter and previous / next. */
function Lightbox({
  urls,
  index,
  title,
  onIndex,
  onError,
  onClose,
}: {
  urls: string[];
  index: number;
  title: string;
  onIndex: (index: number) => void;
  onError: (url: string) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const total = urls.length;
  const url = urls[index] ?? '';
  const step = (delta: number) => onIndex((index + delta + total) % total);

  useEffect(() => {
    closeRef.current?.focus();
  }, []);

  // Esc closes the viewer only (not the page behind it); arrows step; Tab cycles inside.
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
      return;
    }
    if ((event.key === 'ArrowRight' || event.key === 'ArrowLeft') && total > 1) {
      event.preventDefault();
      step(event.key === 'ArrowRight' ? 1 : -1);
      return;
    }
    if (event.key !== 'Tab' || dialogRef.current === null) return;
    const focusable = [...dialogRef.current.querySelectorAll<HTMLElement>('button:not([disabled])')];
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  };

  return createPortal(
    <div className="lightbox">
      <div className="lightbox-scrim" aria-hidden="true" onClick={onClose} />
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label={t.discover.modPage.viewer(title)}
        className="lightbox-dialog"
        onKeyDown={onKeyDown}
      >
        <div className="lightbox-bar">
          <p className="lightbox-counter num" aria-live="polite">
            {t.discover.modPage.counter(index + 1, total)}
          </p>
          <button
            ref={closeRef}
            type="button"
            className="btn btn-ghost btn-icon lightbox-button"
            aria-label={t.discover.modPage.closeViewer}
            title={t.discover.modPage.closeViewer}
            onClick={onClose}
          >
            <Icon name="close" />
          </button>
        </div>
        <div className="lightbox-stage">
          {total > 1 && (
            <button
              type="button"
              className="btn btn-ghost btn-icon lightbox-button lightbox-prev"
              aria-label={t.discover.modPage.previous}
              title={t.discover.modPage.previous}
              onClick={() => step(-1)}
            >
              <Icon name="chevron-left" />
            </button>
          )}
          <img
            key={url}
            className="lightbox-image"
            src={url}
            alt={t.discover.modPage.screenshotAlt(title, index + 1, total)}
            decoding="async"
            referrerPolicy="no-referrer"
            onError={() => onError(url)}
          />
          {total > 1 && (
            <button
              type="button"
              className="btn btn-ghost btn-icon lightbox-button lightbox-next"
              aria-label={t.discover.modPage.next}
              title={t.discover.modPage.next}
              onClick={() => step(1)}
            >
              <Icon name="chevron-right" />
            </button>
          )}
        </div>
      </div>
    </div>,
    document.body,
  );
}

/**
 * Screenshot gallery of the mod page (#115): a 16:9 main image and a roving-tabindex thumbnail
 * strip; the main image opens the lightbox. Images load lazily behind a skeleton of the same box;
 * a screenshot that fails to load is removed, and with none left (or none at all) nothing renders.
 */
export function ScreenshotGallery({ screenshots, title }: { screenshots: string[]; title: string }) {
  const { t } = useI18n();
  const [failed, setFailed] = useState<ReadonlySet<string>>(() => new Set());
  const [loaded, setLoaded] = useState<ReadonlySet<string>>(() => new Set());
  const [selected, setSelected] = useState<string | null>(null);
  const [viewerOpen, setViewerOpen] = useState(false);
  const stageRef = useRef<HTMLButtonElement>(null);
  const thumbRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const valid = screenshots.filter((url) => !failed.has(url));
  if (valid.length === 0) return null;

  const current = currentShot(screenshots, valid, selected);
  const index = Math.max(0, valid.indexOf(current));
  const total = valid.length;
  const isLoaded = loaded.has(current);

  const markFailed = (url: string) => setFailed((previous) => new Set(previous).add(url));
  const markLoaded = (url: string) => setLoaded((previous) => new Set(previous).add(url));

  const closeViewer = () => {
    setViewerOpen(false);
    stageRef.current?.focus();
  };

  const onThumbKeyDown = (event: KeyboardEvent<HTMLButtonElement>, at: number) => {
    const targets: Record<string, number> = {
      ArrowRight: (at + 1) % total,
      ArrowLeft: (at - 1 + total) % total,
      Home: 0,
      End: total - 1,
    };
    const next = targets[event.key];
    if (next === undefined) return;
    event.preventDefault();
    setSelected(valid[next] ?? null);
    thumbRefs.current[next]?.focus();
  };

  return (
    <section className="gallery" aria-label={t.discover.modPage.gallery}>
      <button
        ref={stageRef}
        type="button"
        className="gallery-stage"
        aria-haspopup="dialog"
        title={t.discover.modPage.viewFullSize}
        data-loading={isLoaded ? undefined : 'true'}
        onClick={() => setViewerOpen(true)}
      >
        {!isLoaded && <span className="skeleton gallery-skeleton" aria-hidden="true" />}
        <img
          key={current}
          className="gallery-image"
          src={current}
          alt={t.discover.modPage.screenshotAlt(title, index + 1, total)}
          {...REMOTE_IMAGE}
          onLoad={() => markLoaded(current)}
          onError={() => markFailed(current)}
        />
      </button>
      {total > 1 && (
        <div role="group" aria-label={t.discover.modPage.thumbnails} className="gallery-thumbs">
          {valid.map((url, at) => (
            <button
              key={url}
              ref={(element) => {
                thumbRefs.current[at] = element;
              }}
              type="button"
              className="gallery-thumb"
              aria-pressed={url === current}
              aria-label={t.discover.modPage.showScreenshot(at + 1, total)}
              tabIndex={url === current ? 0 : -1}
              data-loading={loaded.has(url) ? undefined : 'true'}
              onClick={() => setSelected(url)}
              onKeyDown={(event) => onThumbKeyDown(event, at)}
            >
              <img src={url} alt="" {...REMOTE_IMAGE} onLoad={() => markLoaded(url)} onError={() => markFailed(url)} />
            </button>
          ))}
        </div>
      )}
      {viewerOpen && (
        <Lightbox
          urls={valid}
          index={index}
          title={title}
          onIndex={(next) => setSelected(valid[next] ?? null)}
          onError={markFailed}
          onClose={closeViewer}
        />
      )}
    </section>
  );
}
