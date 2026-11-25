import type { Catalog } from '../../domain';
import { toCommandError } from '../../domain';
import type { CatalogController, CatalogDeps, CatalogState } from '../catalogController';

export function createCatalogController(deps: CatalogDeps): CatalogController {
  let state: CatalogState = { status: 'idle', catalog: null, error: null, refreshing: false, pageError: null };
  const listeners = new Set<() => void>();
  let inFlight: Promise<void> | null = null;

  function set(patch: Partial<CatalogState>): void {
    state = { ...state, ...patch };
    for (const listener of [...listeners]) listener();
  }

  /** One request at a time: a call made while another is in flight waits for that one. */
  function fetchWith(request: () => Promise<Catalog>): Promise<void> {
    if (inFlight !== null) return inFlight;
    if (state.catalog === null) set({ status: 'loading', error: null });
    else set({ refreshing: true, error: null });
    inFlight = request()
      .then(
        (catalog) => set({ status: 'ready', catalog, error: null, refreshing: false }),
        (caught: unknown) => {
          const error = toCommandError(caught);
          set(state.catalog === null ? { status: 'error', error } : { error, refreshing: false });
        },
      )
      .finally(() => {
        inFlight = null;
      });
    return inFlight;
  }

  return {
    getState: () => state,
    subscribe(listener) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    load() {
      if (state.status !== 'idle') return inFlight ?? Promise.resolve();
      return fetchWith(() => deps.gateway.getCatalog());
    },
    refresh() {
      return fetchWith(() => deps.gateway.refreshCatalog());
    },
    async openPage(id) {
      try {
        await deps.gateway.openCatalogPage(id);
        if (state.pageError !== null) set({ pageError: null });
      } catch (caught) {
        set({ pageError: toCommandError(caught) });
      }
    },
    dismissError() {
      if (state.error !== null && state.status !== 'error') set({ error: null });
    },
    dismissPageError() {
      if (state.pageError !== null) set({ pageError: null });
    },
  };
}
