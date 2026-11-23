export interface AppInfo {
  /** The running launcher version (Tauri app version). */
  getVersion(): Promise<string>;
}
