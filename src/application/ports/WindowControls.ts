import type { Unsubscribe } from '../ports';

/**
 * The launcher window's own controls, used by the custom title bar (issue #69) because the
 * native decorations are turned off. Every async call may reject; callers swallow the failure.
 */
export interface WindowControls {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  close(): Promise<void>;
  isMaximized(): Promise<boolean>;
  /** Calls the listener with the new state whenever the window may have been maximized or restored. */
  onMaximizedChange(listener: (maximized: boolean) => void): Unsubscribe;
}
