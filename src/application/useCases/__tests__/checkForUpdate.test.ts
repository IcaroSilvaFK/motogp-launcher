import { describe, expect, it } from 'vitest';
import type { UpdateInfo } from '../../../domain';
import { FakeUpdateService } from '../../__tests__/fakeUpdateService';
import { createUpdateController } from '../checkForUpdate';

const NEWER: UpdateInfo = { version: '0.2.0', currentVersion: '0.1.0', notes: 'n', date: null };
const SAME: UpdateInfo = { version: '0.1.0', currentVersion: '0.1.0', notes: null, date: null };
const OLDER: UpdateInfo = { version: '0.0.9', currentVersion: '0.1.0', notes: null, date: null };

function setup(clean = true) {
  const service = new FakeUpdateService();
  const flags = { clean };
  const controller = createUpdateController({ service, isSessionClean: () => flags.clean });
  return { service, flags, controller };
}

describe('checkForUpdate', () => {
  it('starts idle', () => {
    expect(setup().controller.getState()).toEqual({ kind: 'idle' });
  });

  it('checkForUpdate_returns_new_release', async () => {
    const { service, controller } = setup();
    service.checkResult = NEWER;
    await controller.check();
    expect(controller.getState()).toEqual({ kind: 'available', info: NEWER });
  });

  it('checkForUpdate_returns_none_when_current', async () => {
    const { service, controller } = setup();
    service.checkResult = null;
    await controller.check();
    expect(controller.getState()).toEqual({ kind: 'none' });
    service.checkResult = SAME;
    await controller.check();
    expect(controller.getState()).toEqual({ kind: 'none' });
  });

  it('older_release_is_not_offered', async () => {
    const { service, controller } = setup();
    service.checkResult = OLDER;
    await controller.check();
    expect(controller.getState()).toEqual({ kind: 'none' });
  });

  it('check_failure_does_not_block_launcher', async () => {
    const { service, controller } = setup();
    service.checkError = { kind: 'network', message: 'offline' };
    await expect(controller.check()).resolves.toBeUndefined();
    expect(controller.getState()).toEqual({
      kind: 'error',
      info: null,
      error: { kind: 'network', message: 'offline' },
    });
  });

  it('maps unknown check failures to unknown', async () => {
    const { service, controller } = setup();
    service.checkError = new Error('boom');
    await controller.check();
    expect(controller.getState()).toEqual({
      kind: 'error',
      info: null,
      error: { kind: 'unknown', message: 'boom' },
    });
    service.checkError = 'weird';
    await controller.check();
    expect(controller.getState()).toMatchObject({ error: { kind: 'unknown', message: 'weird' } });
  });

  it('download_progress_is_forwarded', async () => {
    const { service, controller } = setup();
    service.checkResult = NEWER;
    service.progressEvents = [
      { kind: 'started', total: 100 },
      { kind: 'progress', chunk: 30 },
      { kind: 'progress', chunk: 20 },
    ];
    service.holdDownload();
    await controller.check();
    let notified = 0;
    controller.subscribe(() => (notified += 1));
    const pending = controller.install();
    await Promise.resolve();
    expect(controller.getState()).toEqual({
      kind: 'downloading',
      info: NEWER,
      downloaded: 50,
      total: 100,
    });
    expect(notified).toBeGreaterThan(0);
    service.releaseDownload();
    await pending;
  });

  it('keeps total null when unknown and ignores finished', async () => {
    const { service, controller } = setup();
    service.checkResult = NEWER;
    service.progressEvents = [{ kind: 'started', total: null }, { kind: 'progress', chunk: 5 }, { kind: 'finished' }];
    service.holdDownload();
    await controller.check();
    const pending = controller.install();
    await Promise.resolve();
    expect(controller.getState()).toMatchObject({ downloaded: 5, total: null });
    service.releaseDownload();
    await pending;
  });

  it('restart_only_after_successful_install', async () => {
    const { service, controller } = setup();
    service.checkResult = NEWER;
    service.holdDownload();
    await controller.check();
    const pending = controller.install();
    await Promise.resolve();
    expect(service.restartCalls).toBe(0);
    service.releaseDownload();
    await pending;
    expect(service.restartCalls).toBe(1);
    expect(controller.getState()).toEqual({ kind: 'installed', info: NEWER });
  });

  it('install_failure_does_not_restart', async () => {
    const { service, controller } = setup();
    service.checkResult = NEWER;
    service.installError = { kind: 'signature', message: 'bad sig' };
    await controller.check();
    await expect(controller.install()).resolves.toBeUndefined();
    expect(service.restartCalls).toBe(0);
    expect(controller.getState()).toEqual({
      kind: 'error',
      info: NEWER,
      error: { kind: 'signature', message: 'bad sig' },
    });
  });

  it('can retry install from error with info', async () => {
    const { service, controller } = setup();
    service.checkResult = NEWER;
    service.installError = { kind: 'network', message: 'x' };
    await controller.check();
    await controller.install();
    service.installError = null;
    await controller.install();
    expect(service.downloadCalls).toBe(2);
    expect(controller.getState()).toEqual({ kind: 'installed', info: NEWER });
  });

  it('restart failure becomes an error', async () => {
    const { service, controller } = setup();
    service.checkResult = NEWER;
    service.restartError = new Error('no restart');
    await controller.check();
    await controller.install();
    expect(controller.getState()).toEqual({
      kind: 'error',
      info: NEWER,
      error: { kind: 'unknown', message: 'no restart' },
    });
  });

  it('concurrent_update_is_rejected', async () => {
    const { service, controller } = setup();
    service.checkResult = NEWER;
    service.holdDownload();
    await controller.check();
    const first = controller.install();
    await controller.install();
    expect(service.downloadCalls).toBe(1);
    service.releaseDownload();
    await first;
    await controller.install();
    expect(service.downloadCalls).toBe(1);
  });

  it('install is a no-op without an available update', async () => {
    const { service, controller } = setup();
    await controller.install();
    service.checkError = new Error('x');
    await controller.check();
    await controller.install();
    expect(service.downloadCalls).toBe(0);
  });

  it('active_or_unclean_session_blocks_install', async () => {
    const { service, flags, controller } = setup(false);
    service.checkResult = NEWER;
    await controller.check();
    await controller.install();
    expect(service.downloadCalls).toBe(0);
    expect(service.restartCalls).toBe(0);
    expect(controller.getState()).toMatchObject({
      kind: 'error',
      info: NEWER,
      error: { kind: 'session' },
    });
    flags.clean = true;
    await controller.install();
    expect(service.downloadCalls).toBe(1);
  });

  it('subscribe returns a working unsubscribe', async () => {
    const { service, controller } = setup();
    let count = 0;
    const off = controller.subscribe(() => (count += 1));
    await controller.check();
    const after = count;
    expect(after).toBeGreaterThan(0);
    off();
    service.checkResult = NEWER;
    await controller.check();
    expect(count).toBe(after);
  });
});
