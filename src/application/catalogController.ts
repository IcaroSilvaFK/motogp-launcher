import type { Catalog, CommandError } from '../domain';
import type { LauncherGateway } from './ports';

/** The catalog commands the Discover tab needs (#73). */
export type CatalogGateway = Pick<LauncherGateway, 'getCatalog' | 'refreshCatalog' | 'openCatalogPage'>;

export interface CatalogState {
  /** `idle` until the first visit to Discover; `error` only when no catalog could be shown. */
  status: 'idle' | 'loading' | 'ready' | 'error';
  catalog: Catalog | null;
  /** The load error (status `error`), or a refresh failure while a catalog stays shown. */
  error: CommandError | null;
  /** A refresh is in flight while a catalog is shown. */
  refreshing: boolean;
  /** The last Open page failure; cleared by the next successful open or by dismissing it. */
  pageError: CommandError | null;
}

export interface CatalogController {
  getState(): CatalogState;
  /** useSyncExternalStore-compatible: returns the unsubscribe function. */
  subscribe(listener: () => void): () => void;
  /** Loads the catalog on the first call only; later calls (and concurrent ones) do nothing. Never throws. */
  load(): Promise<void>;
  /** Forces a fetch (also the Retry of the error state). Concurrent calls share one request. Never throws. */
  refresh(): Promise<void>;
  /** Opens the entry's page by id (the backend owns the URL). Never throws: a failure sets `pageError`. */
  openPage(id: string): Promise<void>;
  dismissError(): void;
  dismissPageError(): void;
}

export interface CatalogDeps {
  gateway: CatalogGateway;
}
