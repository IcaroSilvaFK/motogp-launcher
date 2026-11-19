import { listen as tauriListen } from '@tauri-apps/api/event';
import type { SessionEvents } from '../application/ports';
import type { SessionErrorEvent, SessionEvent } from '../domain';
import { register } from './subscription';

export type ListenFn = <T>(event: string, handler: (event: { payload: T }) => void) => Promise<() => void>;

export function createTauriSessionEvents(listen: ListenFn = tauriListen): SessionEvents {
  return {
    onSession: (callback) =>
      register<SessionEvent>((deliver) => listen<SessionEvent>('session', (event) => deliver(event.payload)), callback),
    onSessionError: (callback) =>
      register<SessionErrorEvent>(
        (deliver) => listen<SessionErrorEvent>('session-error', (event) => deliver(event.payload)),
        callback,
      ),
  };
}
