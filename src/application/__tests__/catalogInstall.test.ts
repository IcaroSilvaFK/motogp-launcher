import { describe, expect, it } from 'vitest';
import type { CatalogInstallProgress } from '../../domain';
import { createLauncherController } from '../createLauncherController';
import { selectCanInstall } from '../selectCanInstall';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from './fakes';

const ID = 'rebalanced-ai';

async function ready() {
  const gateway = new FakeGateway();
  const session = new FakeSessionEvents();
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session,
    drop: new FakeDropEvents(),
  });
  await controller.start();
  return { gateway, session, controller };
}

function gate(gateway: FakeGateway) {
  let open: () => void = () => undefined;
  gateway.installGate = new Promise<void>((resolve) => (open = resolve));
  return open;
}

const progress = (phase: CatalogInstallProgress['phase'], received: number, total: number) =>
  ({ id: ID, phase, received, total }) satisfies CatalogInstallProgress;

describe('Given the catalog install state', () => {
  it('When started Then it subscribes once and dispose releases it', async () => {
    const { gateway, controller } = await ready();
    expect(gateway.progressSubscribers).toBe(1);
    controller.dispose();
    expect(gateway.progressSubscribers).toBe(0);
  });

  it('When an install is in flight Then progress events walk the phases', async () => {
    const { gateway, controller } = await ready();
    const open = gate(gateway);
    const done = controller.installCatalogEntry(ID);
    expect(controller.getState().catalogInstalls[ID]).toEqual({
      status: 'downloading',
      received: 0,
      total: 0,
      percent: 0,
    });
    gateway.emitProgress(progress('downloading', 1, 3));
    expect(controller.getState().catalogInstalls[ID]).toMatchObject({ status: 'downloading', percent: 33 });
    gateway.emitProgress(progress('downloading', 3, 3));
    expect(controller.getState().catalogInstalls[ID]).toMatchObject({ percent: 100 });
    gateway.emitProgress(progress('verifying', 3, 3));
    expect(controller.getState().catalogInstalls[ID]).toEqual({ status: 'verifying' });
    gateway.emitProgress(progress('importing', 3, 3));
    expect(controller.getState().catalogInstalls[ID]).toEqual({ status: 'importing' });
    open();
    await done;
    expect(controller.getState().catalogInstalls[ID]).toEqual({ status: 'installed', enabled: false });
  });

  it('When the total is unknown or inconsistent Then percent stays within 0-100', async () => {
    const { gateway, controller } = await ready();
    const open = gate(gateway);
    const pending = controller.installCatalogEntry(ID);
    gateway.emitProgress(progress('downloading', 5, 0));
    expect(controller.getState().catalogInstalls[ID]).toMatchObject({ percent: 0 });
    gateway.emitProgress(progress('downloading', 9, 3));
    expect(controller.getState().catalogInstalls[ID]).toMatchObject({ percent: 100 });
    open();
    await pending;
  });

  it('When a progress event has no install in flight Then it is ignored', async () => {
    const { gateway, controller } = await ready();
    gateway.emitProgress(progress('downloading', 1, 2));
    expect(controller.getState().catalogInstalls).toEqual({});
  });

  it('When the install succeeds Then the mods are reloaded and no global error is raised', async () => {
    const { controller } = await ready();
    await controller.installCatalogEntry(ID);
    expect(controller.getState().mods.map((mod) => mod.id)).toEqual([ID]);
    expect(controller.getState().error).toBeNull();
  });

  it('When the mods reload fails after a successful install Then the entry stays installed and the error is global', async () => {
    const { gateway, controller } = await ready();
    gateway.listMods = () => Promise.reject({ code: 'read_failed', message: 'disk' });
    await controller.installCatalogEntry(ID);
    expect(controller.getState().catalogInstalls[ID]).toEqual({ status: 'installed', enabled: false });
    expect(controller.getState().error?.code).toBe('read_failed');
  });

  it('When the install fails Then the entry holds the code and message, not a global error', async () => {
    const { gateway, controller } = await ready();
    gateway.failNext('installCatalogEntry', { code: 'download_network', message: 'offline' });
    await controller.installCatalogEntry(ID);
    expect(controller.getState().catalogInstalls[ID]).toEqual({
      status: 'failed',
      code: 'download_network',
      message: 'offline',
    });
    expect(controller.getState().error).toBeNull();
  });

  it('When a failed entry is installed again Then it retries and installs', async () => {
    const { gateway, controller } = await ready();
    gateway.failNext('installCatalogEntry', { code: 'download_network', message: 'offline' });
    await controller.installCatalogEntry(ID);
    await controller.installCatalogEntry(ID);
    expect(gateway.callsOf('installCatalogEntry')).toHaveLength(2);
    expect(controller.getState().catalogInstalls[ID]).toEqual({ status: 'installed', enabled: false });
  });

  it('When the install is cancelled Then the entry returns to idle, not failed', async () => {
    const { gateway, controller } = await ready();
    gateway.failNext('installCatalogEntry', { code: 'download_cancelled', message: 'cancelled' });
    await controller.installCatalogEntry(ID);
    expect(controller.getState().catalogInstalls[ID]).toEqual({ status: 'idle' });
  });

  it('When a second install is requested while one is in flight Then it is ignored', async () => {
    const { gateway, controller } = await ready();
    const open = gate(gateway);
    const first = controller.installCatalogEntry(ID);
    await controller.installCatalogEntry(ID);
    open();
    await first;
    expect(gateway.callsOf('installCatalogEntry')).toHaveLength(1);
  });

  it('When cancel is requested Then only an in-flight entry reaches the gateway', async () => {
    const { gateway, controller } = await ready();
    await controller.cancelCatalogInstall(ID);
    expect(gateway.callsOf('cancelCatalogInstall')).toHaveLength(0);
    const open = gate(gateway);
    const pending = controller.installCatalogEntry(ID);
    await controller.cancelCatalogInstall(ID);
    expect(gateway.callsOf('cancelCatalogInstall')[0]?.args).toEqual([ID]);
    open();
    await pending;
  });

  it('When cancel fails Then the error is raised globally', async () => {
    const { gateway, controller } = await ready();
    const open = gate(gateway);
    const pending = controller.installCatalogEntry(ID);
    gateway.failNext('cancelCatalogInstall', { code: 'write_failed', message: 'x' });
    await controller.cancelCatalogInstall(ID);
    expect(controller.getState().error?.code).toBe('write_failed');
    open();
    await pending;
  });

  it('When a session is live Then installs are refused and selectCanInstall is false', async () => {
    const { gateway, session, controller } = await ready();
    expect(selectCanInstall(controller.getState())).toBe(true);
    session.emitSession({ state: 'started', cleaned: 0, started_at: 1 });
    expect(selectCanInstall(controller.getState())).toBe(false);
    await controller.installCatalogEntry(ID);
    expect(gateway.callsOf('installCatalogEntry')).toHaveLength(0);
    expect(controller.getState().catalogInstalls).toEqual({});
  });

  it('When registering the progress listener throws Then start still completes', async () => {
    const gateway = new FakeGateway();
    gateway.onCatalogInstallProgress = () => {
      throw new Error('no events');
    };
    const controller = createLauncherController({
      gateway,
      picker: new FakePicker(),
      session: new FakeSessionEvents(),
      drop: new FakeDropEvents(),
    });
    await controller.start();
    expect(controller.getState().status).toBe('ready');
  });
});
