import { invoke } from '@tauri-apps/api/core';
import type { DownloadStats } from '../../application';
import { parseDownloadDaily, parseDownloadTotals } from '../../domain';

/**
 * Download counts over IPC (#120): `report_mod_download`, `get_mod_downloads` and
 * `get_mod_download_daily`. Nothing rejects: a missing command, a malformed answer or an unreachable
 * server reads as `null`, and a failed report is only logged.
 */
export function createTauriDownloadStats(): DownloadStats {
  return {
    async report(id) {
      try {
        await invoke('report_mod_download', { id });
      } catch (error) {
        console.warn('report_mod_download failed', error);
      }
    },
    async totals() {
      try {
        return parseDownloadTotals(await invoke('get_mod_downloads'));
      } catch (error) {
        console.warn('get_mod_downloads failed', error);
        return null;
      }
    },
    async daily(id) {
      try {
        return parseDownloadDaily(await invoke('get_mod_download_daily', { id }));
      } catch (error) {
        console.warn('get_mod_download_daily failed', error);
        return null;
      }
    },
  };
}
