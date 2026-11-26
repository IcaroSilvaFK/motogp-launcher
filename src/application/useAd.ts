import { useCallback, useEffect, useRef, useState } from 'react';
import type { AdSlotId, AdViewModel } from '../domain';
import type { AdProvider } from './ports/AdProvider';

function warn(action: string, slot: AdSlotId, error: unknown) {
  // Diagnostics only: an ad failure is never shown to the user.
  console.warn(`ad ${action} failed for slot "${slot}"`, error);
}

/** Runs a provider call that must never throw nor reject into the UI. */
function quietly(action: string, slot: AdSlotId, run: () => Promise<unknown>) {
  try {
    run().catch((error: unknown) => warn(action, slot, error));
  } catch (error) {
    warn(action, slot, error);
  }
}

export interface AdHandle {
  /** The creative to show; `null` while loading, when disabled and when there is none. */
  ad: AdViewModel | null;
  /** Sends the click for the current creative (slot + id only). */
  click(): void;
  /** Reports the current creative as shown, at most once per creative for this mount. */
  shown(): void;
}

/**
 * Loads the creative of one slot in an effect, so nothing ever awaits it (Play and the screens
 * render first). Without a provider, or when `enabled` is false, nothing is requested.
 */
export function useAd(provider: AdProvider | undefined, slot: AdSlotId, enabled: boolean): AdHandle {
  const [loaded, setLoaded] = useState<AdViewModel | null>(null);
  const reported = useRef(new Set<string>());
  const active = provider !== undefined && enabled;
  const ad = active ? loaded : null;

  useEffect(() => {
    if (!active) return;
    let current = true;
    const accept = (next: AdViewModel | null) => {
      if (current) setLoaded(next);
    };
    try {
      provider.get(slot).then(accept, (error: unknown) => {
        warn('request', slot, error);
        accept(null);
      });
    } catch (error) {
      warn('request', slot, error);
    }
    return () => {
      current = false;
      setLoaded(null);
    };
  }, [active, provider, slot]);

  const id = ad?.id ?? null;

  const click = useCallback(() => {
    if (provider === undefined || id === null) return;
    quietly('click', slot, () => provider.click(slot, id));
  }, [provider, slot, id]);

  const shown = useCallback(() => {
    if (provider === undefined || id === null || reported.current.has(id)) return;
    reported.current.add(id);
    quietly('shown', slot, () => provider.reportShown(slot, id));
  }, [provider, slot, id]);

  return { ad, click, shown };
}
