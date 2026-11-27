import { invoke } from '@tauri-apps/api/core';
import type { Diagnostics } from '../../application';

/** Diagnostics over IPC (#148): `get_diagnostics`, `open_logs_folder` and the WebView clipboard. */
export function createTauriDiagnostics(): Diagnostics {
  return {
    async snapshot() {
      const raw = await invoke<{ os?: unknown; logLines?: unknown }>('get_diagnostics');
      const logLines = Array.isArray(raw.logLines) ? raw.logLines.filter((l) => typeof l === 'string') : [];
      return { os: typeof raw.os === 'string' ? raw.os : 'unknown', logLines };
    },
    async openLogsFolder() {
      await invoke('open_logs_folder');
    },
    async copyText(text) {
      if (typeof navigator === 'undefined' || !navigator.clipboard) {
        throw new Error('the clipboard is not available');
      }
      await navigator.clipboard.writeText(text);
    },
  };
}
