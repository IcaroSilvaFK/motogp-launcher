import { useCallback, useEffect, useRef, useState } from 'react';
import { toPublishError } from '../domain';
import type { MyMod, PublishError } from '../domain';
import type { ModPublisher } from './ports/ModPublisher';

/** `loading`: first read in flight; `ready`: `mods` is usable (maybe empty); `error`: the read failed. */
export type MyModsStatus = 'loading' | 'ready' | 'error';

/**
 * What `useMyMods` returns. Typical UI: a skeleton while `loading`, `error.code` translated (with a
 * retry button calling `refresh`) while `error`, else the list; `refreshing` is true while a later read
 * runs, and the previous `mods` stay visible meanwhile.
 */
export interface MyModsHandle {
  /** False without a port: render nothing. */
  available: boolean;
  status: MyModsStatus;
  mods: MyMod[];
  error: PublishError | null;
  refreshing: boolean;
  /** Reads the list again (also the "retry" of the error state). */
  refresh(): void;
}

/** Loads the signed-in creator's mods in an effect. Re-reads on `refresh()` or when the port changes. */
export function useMyMods(port: ModPublisher | undefined): MyModsHandle {
  const [status, setStatus] = useState<MyModsStatus>('loading');
  const [mods, setMods] = useState<MyMod[]>([]);
  const [error, setError] = useState<PublishError | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [reads, setReads] = useState(0);
  const first = useRef(true);

  useEffect(() => {
    if (!port) return;
    let current = true;
    if (first.current) first.current = false;
    else setRefreshing(true);
    let attempt: Promise<MyMod[]>;
    try {
      attempt = port.myMods();
    } catch (thrown) {
      attempt = Promise.reject(thrown);
    }
    attempt.then(
      (list) => {
        if (!current) return;
        setMods(list);
        setError(null);
        setStatus('ready');
        setRefreshing(false);
      },
      (thrown: unknown) => {
        if (!current) return;
        setError(toPublishError(thrown));
        setStatus('error');
        setRefreshing(false);
      },
    );
    return () => {
      current = false;
    };
  }, [port, reads]);

  const refresh = useCallback(() => setReads((n) => n + 1), []);

  return { available: port !== undefined, status, mods, error, refreshing, refresh };
}
