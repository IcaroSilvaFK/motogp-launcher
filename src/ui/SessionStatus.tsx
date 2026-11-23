import type { ReactNode } from 'react';
import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

/** Announces session changes; renders `fallback` (plain, not announced) while there is nothing to say. */
export function SessionStatus({ fallback = null }: { fallback?: ReactNode }) {
  const { t } = useI18n();
  const { state } = useLauncher();
  const { session, error } = state;

  let text: string | null = null;
  switch (session.kind) {
    case 'starting':
      text = t.play.starting;
      break;
    case 'running':
      text = t.session.running;
      break;
    // Never announce cleanup after an error.
    case 'ended':
      text = error === null ? t.session.ended(session.cleaned) : null;
      break;
    case 'never-started':
      text = error === null ? t.session.neverStarted : null;
      break;
    case 'unavailable':
      text = t.session.unavailable;
      break;
    case 'idle':
      text = null;
      break;
  }

  if (text === null) return fallback;
  return (
    <p role="status" className={`session-status session-status-${session.kind}`}>
      {text}
    </p>
  );
}
