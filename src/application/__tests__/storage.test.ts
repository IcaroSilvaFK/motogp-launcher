import { describe, expect, it } from 'vitest';
import type { StorageUsage } from '../../domain';
import { createLauncherController } from '../createLauncherController';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from './fakes';

// An accented Windows profile folder on purpose: real users have them and it must pass through untouched.
const usage: StorageUsage = {
  root: 'C:/Users/Usuário Padrão/AppData/Roaming/motogp-launcher',
  totalBytes: 1035,
  freeBytes: 5000,
  categories: [
    { category: 'mods', bytes: 1000 },
    { category: 'launcher_data', bytes: 30 },
    { category: 'other', bytes: 5 },
  ],
};

function setup() {
  const gateway = new FakeGateway({ storage: usage });
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
  });
  return { gateway, controller };
}

describe('Given the launcher controller and the storage commands', () => {
  it('When the usage is requested Then it passes the gateway result through without touching the state', async () => {
    const { controller, gateway } = setup();
    await controller.start();
    const before = controller.getState();
    await expect(controller.getStorageUsage()).resolves.toEqual(usage);
    expect(gateway.callsOf('getStorageUsage')).toHaveLength(1);
    expect(controller.getState()).toBe(before);
  });

  it('When the measurement fails Then the error is rejected to the caller and no global error is set', async () => {
    const { controller, gateway } = setup();
    await controller.start();
    gateway.failNext('getStorageUsage', { code: 'read_failed', message: 'denied' });
    await expect(controller.getStorageUsage()).rejects.toEqual({ code: 'read_failed', message: 'denied' });
    expect(controller.getState().error).toBeNull();
  });

  it('When the data folder is opened Then the gateway is called with no path', async () => {
    const { controller, gateway } = setup();
    await controller.openDataFolder();
    expect(gateway.callsOf('openDataFolder')).toEqual([{ method: 'openDataFolder', args: [] }]);
  });

  it('When opening the folder fails Then the error is rejected to the caller and no global error is set', async () => {
    const { controller, gateway } = setup();
    gateway.failNext('openDataFolder', { code: 'task_failed', message: 'x' });
    await expect(controller.openDataFolder()).rejects.toEqual({ code: 'task_failed', message: 'x' });
    expect(controller.getState().error).toBeNull();
  });

  it('When the gateway rejects with a raw value Then it is normalized to a CommandError', async () => {
    const { controller, gateway } = setup();
    gateway.getStorageUsage = () => Promise.reject(new Error('boom'));
    await expect(controller.getStorageUsage()).rejects.toEqual({ code: 'unknown', message: 'boom' });
  });
});
