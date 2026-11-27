import { describe, expect, it, vi } from 'vitest';
import { createTauriDropEvents } from '../tauriDropEvents';
import { createTauriSessionEvents } from '../tauriSessionEvents';

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

type Handler = (event: { payload: never }) => void;

/** A fake of the SDK registration call; `resolve` completes it (immediately by default). */
function fakeRegistration(deferred = false) {
  let handler: Handler = () => undefined;
  const unlisten = vi.fn();
  let release: () => void = () => undefined;
  const gate = new Promise<void>((resolve) => (release = resolve));
  const register = vi.fn((_name: string, h: Handler) => {
    handler = h;
    return deferred ? gate.then(() => unlisten) : Promise.resolve(unlisten);
  });
  return { register, unlisten, release, emit: (payload: unknown) => handler({ payload } as { payload: never }) };
}

function webviewOf(fake: ReturnType<typeof fakeRegistration>) {
  return () => ({
    onDragDropEvent: (h: Handler) => fake.register('drag-drop', h),
  });
}

describe('Given the session and drop adapters', () => {
  it('When subscribed Then session_and_drop_adapters_unsubscribe', async () => {
    const session = fakeRegistration();
    const events = createTauriSessionEvents(session.register);
    const onSession = vi.fn();
    const off = await events.onSession(onSession);
    session.emit({ type: 'started' });
    expect(onSession).toHaveBeenCalledWith({ type: 'started' });
    expect(session.register.mock.calls[0]?.[0]).toBe('session');
    await Promise.resolve();
    off();
    await flush();
    off();
    await flush();
    expect(session.unlisten).toHaveBeenCalledTimes(1);
    session.emit({ type: 'ended' });
    expect(onSession).toHaveBeenCalledTimes(1);

    const errors = fakeRegistration();
    const onError = vi.fn();
    const offError = await createTauriSessionEvents(errors.register).onSessionError(onError);
    errors.emit({ message: 'x' });
    expect(errors.register.mock.calls[0]?.[0]).toBe('session-error');
    expect(onError).toHaveBeenCalledWith({ message: 'x' });
    await Promise.resolve();
    offError();
    await flush();
    expect(errors.unlisten).toHaveBeenCalledTimes(1);

    const drop = fakeRegistration();
    const onDrop = vi.fn();
    const offDrop = await createTauriDropEvents(webviewOf(drop)).onDrop(onDrop);
    drop.emit({ type: 'drop', paths: ['a.pak'] });
    expect(onDrop).toHaveBeenCalledWith(['a.pak']);
    await Promise.resolve();
    offDrop();
    await flush();
    offDrop();
    await flush();
    expect(drop.unlisten).toHaveBeenCalledTimes(1);
    drop.emit({ type: 'drop', paths: ['b.pak'] });
    expect(onDrop).toHaveBeenCalledTimes(1);
  });

  it('When unsubscribed before the SDK registration resolves Then late_unsubscribe_releases_listener', async () => {
    const drop = fakeRegistration(true);
    const onDrop = vi.fn();
    const off = await createTauriDropEvents(webviewOf(drop)).onDrop(onDrop);
    off();
    await flush();
    drop.emit({ type: 'drop', paths: ['late.pak'] });
    expect(drop.unlisten).not.toHaveBeenCalled();
    drop.release();
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(drop.unlisten).toHaveBeenCalledTimes(1);
    expect(onDrop).not.toHaveBeenCalled();
  });

  it('When the SDK registration rejects Then the session subscriptions reject with that reason', async () => {
    const reason = new Error('listen failed');
    const listen = vi.fn(() => Promise.reject(reason));
    const events = createTauriSessionEvents(listen);
    await expect(events.onSession(vi.fn())).rejects.toBe(reason);
    await expect(events.onSessionError(vi.fn())).rejects.toBe(reason);
  });

  it('When non-drop drag events arrive Then drop_ignores_non_drop_events', async () => {
    const drop = fakeRegistration();
    const onDrop = vi.fn();
    await createTauriDropEvents(webviewOf(drop)).onDrop(onDrop);
    drop.emit({ type: 'enter', paths: ['a.pak'] });
    drop.emit({ type: 'over' });
    drop.emit({ type: 'leave' });
    expect(onDrop).not.toHaveBeenCalled();
  });

  it('When drag events arrive Then drag_state_follows_enter_over_leave_and_drop', async () => {
    const drag = fakeRegistration();
    const onDragState = vi.fn();
    const off = await createTauriDropEvents(webviewOf(drag)).onDragState(onDragState);
    drag.emit({ type: 'enter', paths: ['a.pak'] });
    drag.emit({ type: 'over' });
    drag.emit({ type: 'leave' });
    drag.emit({ type: 'enter', paths: ['a.pak'] });
    drag.emit({ type: 'drop', paths: ['a.pak'] });
    expect(onDragState.mock.calls).toEqual([[true], [true], [false], [true], [false]]);
    off();
    await flush();
    expect(drag.unlisten).toHaveBeenCalledTimes(1);
    drag.emit({ type: 'enter', paths: ['b.pak'] });
    expect(onDragState).toHaveBeenCalledTimes(5);
  });
});
