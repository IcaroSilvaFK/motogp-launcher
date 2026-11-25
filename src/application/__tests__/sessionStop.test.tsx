import { describe, expect, it } from 'vitest';
import type { CommandError, CurrentSession, GameInfo } from '../../domain';
import { createLauncherController } from '../createLauncherController';
import { selectCanPlay } from '../selectCanPlay';
import { selectCanStop } from '../selectCanStop';
import { selectProfileLocked } from '../selectProfileLocked';
import { selectSessionClean } from '../selectSessionClean';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from './fakes';
import type { FakeGatewayOptions } from './fakes';

const game: GameInfo = { name: 'MotoGP25', root: 'C:/g', mods_dir: 'C:/g/~mods', app_id: 1234 };

function setup(options: FakeGatewayOptions = {}) {
  const gateway = new FakeGateway({ game, ...options });
  const session = new FakeSessionEvents();
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session,
    drop: new FakeDropEvents(),
  });
  return { gateway, session, controller };
}

async function ready(current?: CurrentSession) {
  const ctx = setup();
  if (current) ctx.gateway.current = current;
  await ctx.controller.start();
  return ctx;
}

/** Makes `stopSession` wait until `settle` is called, to control the order against session events. */
function deferStop(gateway: FakeGateway) {
  let settle: (error?: CommandError) => void = () => undefined;
  gateway.stopSession = () => {
    gateway.calls.push({ method: 'stopSession', args: [] });
    return new Promise<void>((resolve, reject) => {
      settle = (error) => (error ? reject(error) : resolve());
    });
  };
  return { settle: (error?: CommandError) => settle(error) };
}

describe('Given the session events', () => {
  it('When started arrives Then running carries the start time', async () => {
    const { controller, session } = await ready();
    session.emitSession({ state: 'started', cleaned: 0, started_at: 1700 });
    expect(controller.getState().session).toEqual({ kind: 'running', since: 1700 });
  });

  it('When cleaning-up then ended arrive Then both phases are shown in order', async () => {
    const { controller, session } = await ready();
    session.emitSession({ state: 'started', cleaned: 0, started_at: 1 });
    session.emitSession({ state: 'cleaning-up', cleaned: 0, started_at: 1 });
    expect(controller.getState().session).toEqual({ kind: 'cleaning-up' });
    session.emitSession({ state: 'ended', cleaned: 3, started_at: 1 });
    expect(controller.getState().session).toEqual({ kind: 'ended', cleaned: 3 });
  });

  it('When a stop during starting ends without started Then it goes cleaning-up then ended', async () => {
    const { controller, session } = await ready();
    await controller.play();
    session.emitSession({ state: 'cleaning-up', cleaned: 0, started_at: null });
    session.emitSession({ state: 'ended', cleaned: 2, started_at: null });
    expect(controller.getState().session).toEqual({ kind: 'ended', cleaned: 2 });
  });
});

describe('Given selectCanStop and the session selectors', () => {
  it('When the phase varies Then only starting and running can be stopped', async () => {
    const { controller } = await ready();
    const base = controller.getState();
    const can = (next: (typeof base)['session']) => selectCanStop({ ...base, session: next });
    expect(can({ kind: 'starting' })).toBe(true);
    expect(can({ kind: 'running', since: null })).toBe(true);
    expect(can({ kind: 'idle' })).toBe(false);
    expect(can({ kind: 'stopping', since: 1 })).toBe(false);
    expect(can({ kind: 'cleaning-up' })).toBe(false);
    expect(can({ kind: 'ended', cleaned: 1 })).toBe(false);
    expect(can({ kind: 'never-started', cleaned: 0 })).toBe(false);
    expect(can({ kind: 'unavailable' })).toBe(false);
  });

  it.each([{ kind: 'stopping', since: 5 }, { kind: 'cleaning-up' }] as const)(
    'When %o Then it cannot play, is locked and is not clean',
    async (phase) => {
      const { controller } = await ready();
      const state = { ...controller.getState(), session: phase };
      expect(selectCanPlay(state)).toBe(false);
      expect(selectProfileLocked(state)).toBe(true);
      expect(selectSessionClean(state)).toBe(false);
    },
  );
});

describe('Given controller.stopSession', () => {
  it('When running Then it is stopping (keeping since) and calls the gateway', async () => {
    const { controller, session, gateway } = await ready();
    session.emitSession({ state: 'started', cleaned: 0, started_at: 900 });
    await controller.stopSession();
    expect(gateway.callsOf('stopSession')).toHaveLength(1);
    expect(controller.getState().session).toEqual({ kind: 'stopping', since: 900 });
    expect(controller.getState().error).toBeNull();
  });

  it('When starting Then it is stopping without a start time', async () => {
    const { controller, gateway } = await ready();
    await controller.play();
    await controller.stopSession();
    expect(gateway.callsOf('stopSession')).toHaveLength(1);
    expect(controller.getState().session).toEqual({ kind: 'stopping', since: null });
  });

  it.each(['idle', 'cleaning-up', 'ended'] as const)('When %s Then nothing happens', async (kind) => {
    const { controller, session, gateway } = await ready();
    if (kind !== 'idle') session.emitSession({ state: kind, cleaned: 1, started_at: null });
    const before = controller.getState().session;
    await controller.stopSession();
    expect(gateway.callsOf('stopSession')).toHaveLength(0);
    expect(controller.getState().session).toEqual(before);
  });

  it('When already stopping Then a second stop is ignored', async () => {
    const { controller, session, gateway } = await ready();
    session.emitSession({ state: 'started', cleaned: 0, started_at: 1 });
    await controller.stopSession();
    await controller.stopSession();
    expect(gateway.callsOf('stopSession')).toHaveLength(1);
  });

  it('When the stop fails Then the previous phase returns and the error is set', async () => {
    const { controller, session, gateway } = await ready();
    session.emitSession({ state: 'started', cleaned: 0, started_at: 42 });
    gateway.failNext('stopSession', { code: 'stop_failed', message: 'denied' });
    await controller.stopSession();
    expect(controller.getState().session).toEqual({ kind: 'running', since: 42 });
    expect(controller.getState().error).toEqual({ code: 'stop_failed', message: 'denied' });
    // A new attempt is possible.
    await controller.stopSession();
    expect(controller.getState().session).toEqual({ kind: 'stopping', since: 42 });
  });

  it('When the stop fails while starting Then it goes back to starting', async () => {
    const { controller, gateway } = await ready();
    await controller.play();
    gateway.failNext('stopSession', { code: 'no_active_session', message: 'none' });
    await controller.stopSession();
    expect(controller.getState().session).toEqual({ kind: 'starting' });
    expect(controller.getState().error?.code).toBe('no_active_session');
  });

  it('When cleaning-up arrives before the command resolves Then the event state is kept', async () => {
    const { controller, session, gateway } = await ready();
    session.emitSession({ state: 'started', cleaned: 0, started_at: 7 });
    const stop = deferStop(gateway);
    const pending = controller.stopSession();
    session.emitSession({ state: 'cleaning-up', cleaned: 0, started_at: 7 });
    stop.settle();
    await pending;
    expect(controller.getState().session).toEqual({ kind: 'cleaning-up' });
  });

  it('When ended arrives before the command resolves Then ended is kept', async () => {
    const { controller, session, gateway } = await ready();
    session.emitSession({ state: 'started', cleaned: 0, started_at: 7 });
    const stop = deferStop(gateway);
    const pending = controller.stopSession();
    session.emitSession({ state: 'cleaning-up', cleaned: 0, started_at: 7 });
    session.emitSession({ state: 'ended', cleaned: 2, started_at: 7 });
    stop.settle();
    await pending;
    expect(controller.getState().session).toEqual({ kind: 'ended', cleaned: 2 });
  });

  it('When the stop fails after an event moved the phase Then the event state is kept and the error set', async () => {
    const { controller, session, gateway } = await ready();
    session.emitSession({ state: 'started', cleaned: 0, started_at: 7 });
    const stop = deferStop(gateway);
    const pending = controller.stopSession();
    session.emitSession({ state: 'cleaning-up', cleaned: 0, started_at: 7 });
    stop.settle({ code: 'stop_failed', message: 'late' });
    await pending;
    expect(controller.getState().session).toEqual({ kind: 'cleaning-up' });
    expect(controller.getState().error?.code).toBe('stop_failed');
  });

  it('When started arrives while stopping Then it stays stopping with the start time', async () => {
    const { controller, session } = await ready();
    await controller.play();
    await controller.stopSession();
    session.emitSession({ state: 'started', cleaned: 0, started_at: 55 });
    expect(controller.getState().session).toEqual({ kind: 'stopping', since: 55 });
  });
});

describe('Given a reloaded launcher with a live session', () => {
  it.each([
    [{ phase: 'idle', started_at: null }, { kind: 'idle' }],
    [{ phase: 'starting', started_at: null }, { kind: 'starting' }],
    [
      { phase: 'running', started_at: 1234 },
      { kind: 'running', since: 1234 },
    ],
    [
      { phase: 'stopping', started_at: 1234 },
      { kind: 'stopping', since: 1234 },
    ],
    [{ phase: 'cleaning-up', started_at: 1234 }, { kind: 'cleaning-up' }],
  ] as const)('When current_session reports %o Then the phase is %o', async (current, expected) => {
    const { controller, gateway } = await ready(current);
    expect(gateway.callsOf('currentSession')).toHaveLength(1);
    expect(controller.getState().session).toEqual(expected);
    expect(controller.getState().status).toBe('ready');
  });

  it('When a session is live Then play is blocked and a stop is possible', async () => {
    const { controller } = await ready({ phase: 'running', started_at: 3 });
    expect(selectCanPlay(controller.getState())).toBe(false);
    expect(selectCanStop(controller.getState())).toBe(true);
  });

  it('When the session is queried Then it happens after the subscription and recovery', async () => {
    const log: string[] = [];
    const gateway = new FakeGateway({ game, log });
    const controller = createLauncherController({
      gateway,
      picker: new FakePicker(),
      session: new FakeSessionEvents({ log }),
      drop: new FakeDropEvents({ log }),
    });
    await controller.start();
    const recovery = log.indexOf('gateway.recoveryStatus');
    expect(log.indexOf('gateway.currentSession')).toBeGreaterThan(recovery);
    expect(recovery).toBeGreaterThan(0);
  });

  it('When an event arrives during the query Then the older result does not overwrite it', async () => {
    const { controller, gateway, session } = setup();
    gateway.current = { phase: 'running', started_at: 1 };
    const original = gateway.currentSession.bind(gateway);
    gateway.currentSession = async () => {
      const stale = await original();
      session.emitSession({ state: 'ended', cleaned: 4, started_at: 1 });
      return stale;
    };
    await controller.start();
    expect(controller.getState().session).toEqual({ kind: 'ended', cleaned: 4 });
  });

  it('When current_session fails Then loading completes and the error is exposed', async () => {
    const { controller, gateway } = setup();
    gateway.failNext('currentSession', { code: 'task_failed', message: 'boom' });
    await controller.start();
    expect(controller.getState().status).toBe('ready');
    expect(controller.getState().session).toEqual({ kind: 'idle' });
    expect(controller.getState().error).toEqual({ code: 'task_failed', message: 'boom' });
    expect(controller.getState().game).not.toBeNull();
  });

  it('When the session subscription is unavailable Then the phase stays unavailable', async () => {
    const gateway = new FakeGateway({ game });
    gateway.current = { phase: 'running', started_at: 1 };
    const controller = createLauncherController({
      gateway,
      picker: new FakePicker(),
      session: new FakeSessionEvents({ rejectSession: new Error('no ipc') }),
      drop: new FakeDropEvents(),
    });
    await controller.start();
    expect(controller.getState().session).toEqual({ kind: 'unavailable' });
  });
});
