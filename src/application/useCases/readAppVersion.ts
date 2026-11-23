import type { AppInfo } from '../ports/AppInfo';

/** Reads the running launcher version; returns null when unavailable. Never throws. */
export async function readAppVersion(info: AppInfo): Promise<string | null> {
  try {
    const version = (await info.getVersion()).trim();
    return version === '' ? null : version;
  } catch (error) {
    console.warn('Could not read the app version', error);
    return null;
  }
}
