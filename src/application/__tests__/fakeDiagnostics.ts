import type { Diagnostics, DiagnosticsSnapshot } from '../ports/Diagnostics';

/** In-memory Diagnostics port: records what was copied and which folders were opened. */
export class FakeDiagnostics implements Diagnostics {
  copied: string[] = [];
  logsFolderOpened = 0;
  snapshotResult: DiagnosticsSnapshot = { os: 'Windows 11', logLines: [] };
  failCopy = false;
  failOpen = false;

  async snapshot(): Promise<DiagnosticsSnapshot> {
    return this.snapshotResult;
  }

  async openLogsFolder(): Promise<void> {
    if (this.failOpen) throw new Error('no file manager');
    this.logsFolderOpened += 1;
  }

  async copyText(text: string): Promise<void> {
    if (this.failCopy) throw new Error('clipboard unavailable');
    this.copied.push(text);
  }
}
