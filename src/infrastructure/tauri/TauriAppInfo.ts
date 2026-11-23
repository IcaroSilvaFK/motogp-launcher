import { getVersion } from '@tauri-apps/api/app';
import type { AppInfo } from '../../application';

export function createTauriAppInfo(): AppInfo {
  return { getVersion: () => getVersion() };
}
