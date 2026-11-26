import { open as tauriOpen } from '@tauri-apps/plugin-dialog';
import type { PublishFilePicker } from '../application';
import { MAX_SCREENSHOTS, PUBLISH_FILE_EXTENSIONS, SCREENSHOT_EXTENSIONS } from '../domain';
import type { OpenFn } from './tauriPicker';

/** Dialogs of the publish form: one mod file (.zip/.7z/.pak) and up to 8 images. */
export function createTauriPublishPicker(open: OpenFn = tauriOpen as unknown as OpenFn): PublishFilePicker {
  return {
    async pickModFile() {
      const answer = await open({
        multiple: false,
        directory: false,
        filters: [{ name: 'Mods', extensions: [...PUBLISH_FILE_EXTENSIONS] }],
      });
      if (answer === null) return null;
      return Array.isArray(answer) ? (answer[0] ?? null) : answer;
    },
    async pickScreenshots() {
      const answer = await open({
        multiple: true,
        directory: false,
        filters: [{ name: 'Images', extensions: [...SCREENSHOT_EXTENSIONS] }],
      });
      if (answer === null) return null;
      const paths = Array.isArray(answer) ? answer : [answer];
      return paths.slice(0, MAX_SCREENSHOTS);
    },
  };
}
