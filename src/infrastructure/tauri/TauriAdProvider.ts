import { invoke } from '@tauri-apps/api/core';
import type { AdProvider } from '../../application';
import { toAdViewModel } from '../../domain';
import type { AdSlotId } from '../../domain';

/** Runs an invoke and turns a rejection or a synchronous throw into `fallback`, logging it. */
async function safely<T>(command: string, args: Record<string, unknown>, fallback: T, map: (value: unknown) => T) {
  try {
    return map(await invoke(command, args));
  } catch (error) {
    // Expected until the backend ships the command (#103), and offline: diagnostics only.
    console.warn(`${command} failed`, error);
    return fallback;
  }
}

const nothing = () => undefined;

/**
 * Sponsored creatives over IPC (`get_ad`, `ad_clicked`, `ad_shown`). Any failure, a missing
 * command or a malformed answer means "no ad"; click and shown never reject.
 */
export function createTauriAdProvider(): AdProvider {
  return {
    get: (slot: AdSlotId) => safely('get_ad', { slot }, null, toAdViewModel),
    click: (slot, creativeId) => safely('ad_clicked', { slot, creativeId }, undefined, nothing),
    reportShown: (slot, creativeId) => safely('ad_shown', { slot, creativeId }, undefined, nothing),
  };
}
