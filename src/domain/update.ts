export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
  date: string | null;
}

export type UpdateProgress =
  { kind: 'started'; total: number | null } | { kind: 'progress'; chunk: number } | { kind: 'finished' };

export type UpdateErrorKind = 'network' | 'signature' | 'install' | 'busy' | 'session' | 'unknown';

export interface UpdateError {
  kind: UpdateErrorKind;
  message: string;
}

interface ParsedVersion {
  core: [number, number, number];
  prerelease: boolean;
}

const VERSION_PATTERN = /^v?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/;

function parseVersion(raw: string): ParsedVersion | null {
  const match = VERSION_PATTERN.exec(raw.trim());
  if (!match) return null;
  return {
    core: [Number(match[1]), Number(match[2]), Number(match[3])],
    prerelease: match[4] !== undefined,
  };
}

/** Numeric semver comparison; a pre-release ranks below its release; unparseable input is never newer. */
export function isNewerVersion(candidate: string, current: string): boolean {
  const next = parseVersion(candidate);
  const installed = parseVersion(current);
  if (!next || !installed) return false;
  for (const [i, part] of next.core.entries()) {
    const installedPart = installed.core[i] ?? 0;
    if (part !== installedPart) return part > installedPart;
  }
  return installed.prerelease && !next.prerelease;
}
