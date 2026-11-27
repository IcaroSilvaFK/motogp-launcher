import type { Diagnostics, DiagnosticsSnapshot } from '../ports/Diagnostics';

/** The launcher-side facts of a report; the backend adds the OS and the log lines. */
export interface DiagnosticsFacts {
  /** `null` when the version could not be read. */
  version: string | null;
  /** The configured game folder; `null` when none was detected or chosen. */
  gameDir: string | null;
  modCount: number;
  /** Name of the active profile; `null` when there is none. */
  activeProfile: string | null;
}

/** Builds the plain text a user pastes into a bug report. */
export function buildDiagnosticsReport(facts: DiagnosticsFacts, snapshot: DiagnosticsSnapshot): string {
  const lines = [
    'MotoGP Launcher diagnostics',
    `Launcher version: ${facts.version ?? 'unknown'}`,
    `OS: ${snapshot.os}`,
    `Game folder: ${facts.gameDir ?? 'not detected'}`,
    `Installed mods: ${facts.modCount}`,
    `Active profile: ${facts.activeProfile ?? 'none'}`,
    '',
    `Recent log (${snapshot.logLines.length} lines):`,
    ...(snapshot.logLines.length === 0 ? ['(empty)'] : snapshot.logLines),
  ];
  return lines.join('\n');
}

/** Collects the report and copies it. Rejects when the snapshot or the clipboard fails. */
export async function copyDiagnostics(diagnostics: Diagnostics, facts: DiagnosticsFacts): Promise<void> {
  const snapshot = await diagnostics.snapshot();
  await diagnostics.copyText(buildDiagnosticsReport(facts, snapshot));
}
