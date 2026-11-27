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

/** The drag state each Tauri drag-drop event implies; `null` for an unknown event type. */
function dragStateOf(type: string): boolean | null {
  if (type === 'enter' || type === 'over') return true;
  if (type === 'leave' || type === 'drop') return false;
  return null;
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
    onDragState: (callback) =>
      Promise.resolve(
        subscribe<boolean>(
          (deliver) =>
            getWebview().onDragDropEvent((event) => {
              const dragging = dragStateOf(event.payload.type);
              if (dragging !== null) deliver(dragging);
            }),
          callback,
        ),
      ),
  };
}
