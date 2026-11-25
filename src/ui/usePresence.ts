import { useEffect, useState } from 'react';
import { ENTER_DURATION_MS, EXIT_DURATION_MS, prefersReducedMotion } from './motion';

export type PresenceState = 'entering' | 'entered' | 'exiting';

export interface Presence<T> {
  /** What to render: the current value, or the last one while it plays its exit; null when gone. */
  item: T | null;
  /** Goes on the element as `data-state`, which the stylesheet animates. */
  state: PresenceState;
}

/**
 * Keeps a value mounted for a short exit animation after it turns null (issue #71). The element
 * is marked `entering` when it appears, `entered` once the enter has played and `exiting` while it
 * leaves. Under reduced motion (or without matchMedia) there is nothing to wait for, so a null
 * value unmounts at once.
 */
export function usePresence<T>(value: T | null, exitMs: number = EXIT_DURATION_MS): Presence<T> {
  const [item, setItem] = useState<T | null>(value);
  const [state, setState] = useState<PresenceState>('entering');

  // Adjusting state while rendering (not in an effect) avoids one frame with stale content.
  if (value !== null) {
    if (state === 'exiting') setState('entering');
    if (item !== value) setItem(value);
  } else if (item !== null && state !== 'exiting') {
    if (exitMs <= 0 || prefersReducedMotion()) {
      setItem(null);
      setState('entering');
    } else {
      setState('exiting');
    }
  }

  const showing = item !== null;
  useEffect(() => {
    if (!showing) return;
    if (state === 'entering') {
      const timer = setTimeout(() => setState('entered'), ENTER_DURATION_MS);
      return () => clearTimeout(timer);
    }
    if (state === 'exiting') {
      const timer = setTimeout(() => {
        setItem(null);
        setState('entering');
      }, exitMs);
      return () => clearTimeout(timer);
    }
  }, [showing, state, exitMs]);

  return { item, state };
}
