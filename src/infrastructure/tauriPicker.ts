import { open as tauriOpen } from '@tauri-apps/plugin-dialog';
import type { FilePicker } from '../application/ports';
import { SUPPORTED_MOD_EXTENSIONS } from '../domain';

export interface OpenOptions {
  multiple: boolean;
  directory: boolean;
  filters?: { name: string; extensions: string[] }[];
}
export type OpenFn = (options: OpenOptions) => Promise<string | string[] | null>;

export function createTauriPicker(open: OpenFn = tauriOpen as unknown as OpenFn): FilePicker {
  return {
    async pickModFiles() {
      const answer = await open({
        multiple: true,
        directory: false,
        filters: [{ name: 'Mods', extensions: [...SUPPORTED_MOD_EXTENSIONS] }],
      });
      if (answer === null) return null;
      return Array.isArray(answer) ? answer : [answer];
    },
    async pickGameDirectory() {
      const answer = await open({ multiple: false, directory: true });
      if (answer === null) return null;
      return Array.isArray(answer) ? (answer[0] ?? null) : answer;
    },
  };
}
