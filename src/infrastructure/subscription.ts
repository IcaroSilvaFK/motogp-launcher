import type { Unsubscribe } from '../application/ports';

/**
 * Wraps an SDK registration (a promise of an unlisten function) in an idempotent Unsubscribe.
 * The Unsubscribe is returned right away: if it is called before the registration resolves,
 * the unlisten still runs once it does, and the callback is never invoked after unsubscribing.
 */
export function subscribe<T>(
  register: (deliver: (value: T) => void) => Promise<() => void>,
  callback: (value: T) => void,
): Unsubscribe {
  let active = true;
  const registration = register((value) => {
    if (active) callback(value);
  });
  return () => {
    if (!active) return;
    active = false;
    void registration.then(
      (unlisten) => unlisten(),
      () => undefined,
    );
  };
}

/**
 * Like `subscribe`, but resolves only once the SDK registration has completed and rejects with the
 * original reason when it fails, so the caller learns that no event will ever be delivered.
 */
export async function register<T>(
  registerFn: (deliver: (value: T) => void) => Promise<() => void>,
  callback: (value: T) => void,
): Promise<Unsubscribe> {
  let active = true;
  const unlisten = await registerFn((value) => {
    if (active) callback(value);
  });
  return () => {
    if (!active) return;
    active = false;
    unlisten();
  };
}
