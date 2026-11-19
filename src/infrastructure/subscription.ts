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
