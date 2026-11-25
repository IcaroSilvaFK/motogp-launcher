import { useSyncExternalStore } from 'react';
import type { CatalogController, CatalogState } from './catalogController';

export function useCatalogState(controller: CatalogController): CatalogState {
  return useSyncExternalStore(controller.subscribe, controller.getState, controller.getState);
}
