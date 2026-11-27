/** What the backend knows for a diagnostics report: the OS and the recent, already redacted log lines. */
export interface DiagnosticsSnapshot {
  os: string;
  logLines: string[];
}

/** Bug-report helpers (#148): the log file, its folder and the clipboard. */
export interface Diagnostics {
  snapshot(): Promise<DiagnosticsSnapshot>;
  /** Shows the folder that holds the log files; the backend picks the path. */
  openLogsFolder(): Promise<void>;
  /** Puts text on the system clipboard. Rejects when that is not possible. */
  copyText(text: string): Promise<void>;
}
