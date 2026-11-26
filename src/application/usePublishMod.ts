import { useCallback, useEffect, useRef, useState } from 'react';
import { toPublishError, toPublishRequest, validatePublishForm } from '../domain';
import type { ModVersion, PublishErrorCode, PublishForm, PublishFormErrors, PublishProgress } from '../domain';
import type { ModPublisher } from './ports/ModPublisher';
import type { Unsubscribe } from './ports';

/**
 * The publish state machine: `idle` -> `publishing` (with `progress`, `null` until the first event)
 * -> `done` (the stored version, waiting for review) | `failed` (typed `code`, English `message` for
 * logs) | `cancelled`. An invalid form never leaves `idle`: its field errors are in `errors`.
 */
export type PublishState =
  | { status: 'idle' }
  | { status: 'publishing'; progress: PublishProgress | null }
  | { status: 'done'; version: ModVersion }
  | { status: 'failed'; code: PublishErrorCode; message: string }
  | { status: 'cancelled' };

export interface PublishModHandle {
  /** False without a port (tests, builds without the community): render nothing. */
  available: boolean;
  state: PublishState;
  /** Field errors of the last `start` that was refused for an invalid form; `{}` otherwise. */
  errors: PublishFormErrors;
  /** True while `state.status === 'publishing'`. */
  busy: boolean;
  /** Validates, then uploads. Resolves `true` when published; `false` when refused, failed, cancelled or already running. */
  start(form: PublishForm): Promise<boolean>;
  /** Asks the running publish to stop (state becomes `cancelled` once the backend confirms). */
  cancel(): void;
  /** Back to `idle` (and clears `errors`); ignored while publishing. */
  reset(): void;
}

const IDLE: PublishState = { status: 'idle' };

export function usePublishMod(port: ModPublisher | undefined): PublishModHandle {
  const [state, setState] = useState<PublishState>(IDLE);
  const [errors, setErrors] = useState<PublishFormErrors>({});
  const running = useRef(false);
  const mounted = useRef(true);
  const unsubscribe = useRef<Unsubscribe | null>(null);

  const stopListening = useCallback(() => {
    unsubscribe.current?.();
    unsubscribe.current = null;
  }, []);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      stopListening();
    };
  }, [stopListening]);

  const start = useCallback(
    async (form: PublishForm) => {
      if (!port || running.current) return false;
      const found = validatePublishForm(form);
      if (Object.keys(found).length > 0) {
        setErrors(found);
        setState(IDLE);
        return false;
      }
      running.current = true;
      setErrors({});
      setState({ status: 'publishing', progress: null });
      stopListening();
      try {
        unsubscribe.current = port.onProgress((progress) => {
          if (mounted.current) setState({ status: 'publishing', progress });
        });
      } catch {
        // Progress is cosmetic: the upload still runs without it.
      }
      try {
        const version = await port.publish(toPublishRequest(form));
        if (mounted.current) setState({ status: 'done', version });
        return true;
      } catch (thrown) {
        const failure = toPublishError(thrown);
        if (mounted.current) {
          setState(
            failure.code === 'cancelled'
              ? { status: 'cancelled' }
              : { status: 'failed', code: failure.code, message: failure.message },
          );
        }
        return false;
      } finally {
        running.current = false;
        stopListening();
      }
    },
    [port, stopListening],
  );

  const cancel = useCallback(() => {
    if (!port || !running.current) return;
    port.cancel().catch(() => undefined);
  }, [port]);

  const reset = useCallback(() => {
    if (running.current) return;
    setState(IDLE);
    setErrors({});
  }, []);

  return {
    available: port !== undefined,
    state,
    errors,
    busy: state.status === 'publishing',
    start,
    cancel,
    reset,
  };
}
