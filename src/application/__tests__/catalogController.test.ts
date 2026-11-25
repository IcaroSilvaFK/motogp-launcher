import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Catalog, CommandError } from '../../domain';
import type { CatalogGateway } from '../catalogController';
import { createCatalogController } from '../useCases/catalog';

const catalog = (patch: Partial<Catalog> = {}): Catalog => ({
  entries: [],
  fetchedAt: 1,
  stale: false,
  source: 'remote',
  skipped: 0,
  ...patch,
});

const unavailable: CommandError = { code: 'catalog_unavailable', message: 'offline' };

/** A scripted gateway: each call takes the next queued result (a value or an error). */
function scripted() {
  const calls: string[] = [];
  const queue: Record<string, (Catalog | CommandError | undefined)[]> = {
    getCatalog: [],
    refreshCatalog: [],
    openCatalogPage: [],
  };
  const take = <T>(method: string, fallback: T): Promise<T> => {
    calls.push(method);
    const next = queue[method]?.shift();
    if (next !== undefined && 'code' in next) return Promise.reject(next);
    return Promise.resolve((next as T | undefined) ?? fallback);
  };
  const gateway: CatalogGateway = {
    getCatalog: () => take('getCatalog', catalog()),
    refreshCatalog: () => take('refreshCatalog', catalog()),
    openCatalogPage: (id) => take(`openCatalogPage:${id}`, undefined),
  };
  return { gateway, calls, queue };
}

describe('catalog controller', () => {
  it('starts idle', () => {
    const controller = createCatalogController({ gateway: scripted().gateway });
    expect(controller.getState()).toEqual({
      status: 'idle',
      catalog: null,
      error: null,
      refreshing: false,
      pageError: null,
    });
  });

  it('loads once on the first visit and notifies listeners', async () => {
    const { gateway, calls } = scripted();
    const controller = createCatalogController({ gateway });
    const seen: string[] = [];
    const unsubscribe = controller.subscribe(() => seen.push(controller.getState().status));
    const first = controller.load();
    expect(controller.getState().status).toBe('loading');
    const second = controller.load();
    await Promise.all([first, second]);
    await controller.load();
    expect(calls).toEqual(['getCatalog']);
    expect(controller.getState().status).toBe('ready');
    expect(seen).toEqual(['loading', 'ready']);
    unsubscribe();
    await controller.refresh();
    expect(seen).toEqual(['loading', 'ready']);
  });

  it('enters the error state when the first load fails, and retry refreshes', async () => {
    const { gateway, calls, queue } = scripted();
    queue.getCatalog?.push(unavailable);
    const controller = createCatalogController({ gateway });
    await controller.load();
    expect(controller.getState()).toMatchObject({ status: 'error', error: unavailable, catalog: null });
    await controller.load();
    expect(calls).toEqual(['getCatalog']);
    const fresh = catalog({ fetchedAt: 5 });
    queue.refreshCatalog?.push(fresh);
    await controller.refresh();
    expect(controller.getState()).toMatchObject({ status: 'ready', catalog: fresh, error: null });
    expect(calls).toEqual(['getCatalog', 'refreshCatalog']);
  });

  it('keeps the shown catalog while refreshing and when a refresh fails', async () => {
    const { gateway, queue } = scripted();
    const shown = catalog({ fetchedAt: 2 });
    queue.getCatalog?.push(shown);
    const controller = createCatalogController({ gateway });
    await controller.load();
    queue.refreshCatalog?.push(unavailable);
    const pending = controller.refresh();
    expect(controller.getState()).toMatchObject({ status: 'ready', refreshing: true });
    await controller.refresh();
    await pending;
    expect(controller.getState()).toMatchObject({
      status: 'ready',
      catalog: shown,
      refreshing: false,
      error: unavailable,
    });
    controller.dismissError();
    expect(controller.getState().error).toBeNull();
  });

  it('wraps unknown failures into a command error', async () => {
    const controller = createCatalogController({
      gateway: {
        getCatalog: () => Promise.reject(new Error('boom')),
        refreshCatalog: () => Promise.reject(new Error('boom')),
        openCatalogPage: () => Promise.reject(new Error('boom')),
      },
    });
    await controller.load();
    expect(controller.getState().error?.code).toBeDefined();
    await controller.openPage('x');
    expect(controller.getState().pageError?.message).toContain('boom');
  });

  it('opens a page by id and reports a failure as a page error', async () => {
    const { gateway, calls, queue } = scripted();
    const controller = createCatalogController({ gateway });
    await controller.openPage('rebalanced-ai');
    expect(calls).toEqual(['openCatalogPage:rebalanced-ai']);
    expect(controller.getState().pageError).toBeNull();
    const notAllowed: CommandError = { code: 'catalog_page_not_allowed', message: '' };
    queue['openCatalogPage:x'] = [notAllowed];
    await controller.openPage('x');
    expect(controller.getState().pageError).toEqual(notAllowed);
    await controller.openPage('rebalanced-ai');
    expect(controller.getState().pageError).toBeNull();
    queue['openCatalogPage:x'] = [notAllowed];
    await controller.openPage('x');
    controller.dismissPageError();
    expect(controller.getState().pageError).toBeNull();
    controller.dismissPageError();
    controller.dismissError();
  });
});

describe('catalog controller logging', () => {
  afterEach(() => vi.restoreAllMocks());

  it('logs the English backend message of a failure instead of leaving it for the screen', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const { gateway, queue } = scripted();
    queue.getCatalog?.push({ code: 'catalog_not_published', message: 'HTTP 404' });
    queue.refreshCatalog?.push(unavailable);
    const controller = createCatalogController({ gateway });

    await controller.load();
    await controller.refresh();

    expect(warn).toHaveBeenCalledWith('Catalog request failed (catalog_not_published): HTTP 404');
    expect(warn).toHaveBeenCalledWith('Catalog request failed (catalog_unavailable): offline');
  });

  it('logs a failure to open a page', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const { gateway, queue } = scripted();
    queue['openCatalogPage:x'] = [{ code: 'open_page_failed', message: 'no browser' }];
    const controller = createCatalogController({ gateway });

    await controller.openPage('x');

    expect(warn).toHaveBeenCalledWith('Catalog request failed (open_page_failed): no browser');
  });
});
