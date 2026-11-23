import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { DropEvents } from '../application/ports';
import { subscribe } from './subscription';

interface DragDropPayload {
  type: string;
  paths?: string[];
}
export interface DragDropWebview {
  onDragDropEvent(handler: (event: { payload: DragDropPayload }) => void): Promise<() => void>;
}

export function createTauriDropEvents(getWebview: () => DragDropWebview = getCurrentWebview): DropEvents {
  return {
    onDrop: (callback) =>
      Promise.resolve(
        subscribe<string[]>(
          (deliver) =>
            getWebview().onDragDropEvent((event) => {
              if (event.payload.type === 'drop') deliver(event.payload.paths ?? []);
            }),
          callback,
        ),
      ),
  };
}
