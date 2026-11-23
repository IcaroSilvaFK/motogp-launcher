import { useI18n } from './i18n';
import { useLauncher } from './LauncherContext';

export function SessionStatus() {
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

  if (text === null) return null;
  return (
    <p role="status" className="session-status">
      {text}
    </p>
  );
}
