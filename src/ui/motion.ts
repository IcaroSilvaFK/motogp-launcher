import type { CSSProperties } from 'react';

/**
 * Motion constants shared by the UI (issue #71). The CSS tokens in styles.css are the source of
 * truth for the visuals; these numbers only mirror the ones JavaScript has to wait on or cap.
 */

/** Items after this index enter together with the last staggered one (`--motion-stagger-step` apart). */
export const STAGGER_CAP = 8;

/** How long a banner or notice stays mounted while it plays its exit; mirrors `--motion-duration-short4`. */
export const EXIT_DURATION_MS = 200;

/** How long a banner or notice is marked as entering; mirrors `--motion-duration-medium1`. */
export const ENTER_DURATION_MS = 250;

/** Inline style that places a list item in the staggered entrance (`.stagger-item`), capped. */
export function stagger(index: number): CSSProperties {
  return { '--stagger-index': Math.min(Math.max(index, 0), STAGGER_CAP) } as CSSProperties;
}

/** True when the user asked the OS for reduced motion, or the query cannot be made at all. */
export function prefersReducedMotion(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return true;
  return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}
