// The optional `onCatalogInstalled` dependency (#120): the download report fires only after a
// successful install, is never awaited and can never break the install.
import { describe, expect, it, vi } from 'vitest';
import { createLauncherController } from '../createLauncherController';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from './fakes';

const ID = 'rebalanced-ai';

async function ready(onCatalogInstalled?: (id: string) => void) {
  const gateway = new FakeGateway();
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
    onCatalogInstalled,
  });
  await controller.start();
  return { gateway, controller };
}

describe('Given a catalog install with an install hook', () => {
  it('When the install succeeds Then the hook is called once with the entry id', async () => {
    const hook = vi.fn();
    const { controller } = await ready(hook);
    await controller.installCatalogEntry(ID);
    expect(hook).toHaveBeenCalledTimes(1);
    expect(hook).toHaveBeenCalledWith(ID);
  });

  it('When the install fails Then the hook is not called', async () => {
    const hook = vi.fn();
    const { gateway, controller } = await ready(hook);
    gateway.failNext('installCatalogEntry', { code: 'download_network', message: 'offline' });
    await controller.installCatalogEntry(ID);
    expect(hook).not.toHaveBeenCalled();
  });

  it('When the install is cancelled Then the hook is not called', async () => {
    const hook = vi.fn();
    const { gateway, controller } = await ready(hook);
    gateway.failNext('installCatalogEntry', { code: 'download_cancelled', message: 'cancelled' });
    await controller.installCatalogEntry(ID);
    expect(hook).not.toHaveBeenCalled();
  });

  it('When the hook throws Then the install still completes and the mods reload', async () => {
    const { gateway, controller } = await ready(() => {
      throw new Error('boom');
    });
    const before = gateway.callsOf('listMods').length;
    await controller.installCatalogEntry(ID);
    expect(controller.getState().catalogInstalls[ID]).toEqual({ status: 'installed', enabled: false });
    expect(controller.getState().error).toBeNull();
    expect(gateway.callsOf('listMods').length).toBe(before + 1);
  });

  it('When the hook never settles Then the install does not wait for it', async () => {
    const { controller } = await ready(() => void new Promise(() => undefined));
    await controller.installCatalogEntry(ID);
    expect(controller.getState().catalogInstalls[ID]).toEqual({ status: 'installed', enabled: false });
  });

  it('When no hook is wired Then the install works as before', async () => {
    const { controller } = await ready();
    await controller.installCatalogEntry(ID);
    expect(controller.getState().catalogInstalls[ID]).toEqual({ status: 'installed', enabled: false });
  });
});
