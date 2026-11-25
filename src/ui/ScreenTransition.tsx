import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';

/**
 * Wraps the current screen so it fades and slides in on mount (issue #71). Enter only: the old
 * screen leaves at once, so navigation never waits on an exit. Give it a `key` per screen.
 */
export function ScreenTransition({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const [entered, setEntered] = useState(false);

  // A native listener: React's onAnimationEnd depends on vendor-prefix detection.
  useEffect(() => {
    const node = ref.current;
    if (node === null) return;
    // Rows inside run their own entrance; only the wrapper's own animation ends the enter.
    const onEnd = (event: Event) => {
      if (event.target === node) setEntered(true);
    };
    node.addEventListener('animationend', onEnd);
    return () => node.removeEventListener('animationend', onEnd);
  }, []);

  return (
    <div ref={ref} className="screen" data-motion="screen" data-state={entered ? 'entered' : 'entering'}>
      {children}
    </div>
  );
}
