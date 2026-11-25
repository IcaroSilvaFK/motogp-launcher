import { beforeEach, describe, expect, it, vi } from 'vitest';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { createTauriWindowControls } from '../tauri/TauriWindowControls';

vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: vi.fn() }));

type ResizedHandler = () => void;

function fakeWindow() {
  const handlers: ResizedHandler[] = [];
  const unlisten = vi.fn();
  const win = {
    minimize: vi.fn(() => Promise.resolve()),
    toggleMaximize: vi.fn(() => Promise.resolve()),
    close: vi.fn(() => Promise.resolve()),
    isMaximized: vi.fn(() => Promise.resolve(false)),
    onResized: vi.fn((handler: ResizedHandler) => {
      handlers.push(handler);
      return Promise.resolve(unlisten);
    }),
  };
  return { win, handlers, unlisten, resize: () => handlers.forEach((handler) => handler()) };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

let current: ReturnType<typeof fakeWindow>;

beforeEach(() => {
  current = fakeWindow();
  vi.mocked(getCurrentWindow).mockReturnValue(current.win as unknown as ReturnType<typeof getCurrentWindow>);
});

describe('createTauriWindowControls', () => {
  it('forwards minimize, toggleMaximize and close to the current window', async () => {
    const controls = createTauriWindowControls();
    await controls.minimize();
    await controls.toggleMaximize();
    await controls.close();
    expect(current.win.minimize).toHaveBeenCalledTimes(1);
    expect(current.win.toggleMaximize).toHaveBeenCalledTimes(1);
    expect(current.win.close).toHaveBeenCalledTimes(1);
  });

  it('reads whether the window is maximized', async () => {
    current.win.isMaximized.mockResolvedValueOnce(true);
    expect(await createTauriWindowControls().isMaximized()).toBe(true);
  });

  it('propagates a rejection', async () => {
    current.win.close.mockRejectedValueOnce(new Error('no window'));
    await expect(createTauriWindowControls().close()).rejects.toThrow('no window');
  });

  it('re-reads the maximized state on every resize until unsubscribed', async () => {
    const listener = vi.fn();
    const unsubscribe = createTauriWindowControls().onMaximizedChange(listener);
    await flush();
    current.win.isMaximized.mockResolvedValueOnce(true);
    current.resize();
    await flush();
    expect(listener).toHaveBeenLastCalledWith(true);

    unsubscribe();
    await flush();
    expect(current.unlisten).toHaveBeenCalledTimes(1);
    current.resize();
    await flush();
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('ignores a failed state read after a resize', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const listener = vi.fn();
    createTauriWindowControls().onMaximizedChange(listener);
    await flush();
    current.win.isMaximized.mockRejectedValueOnce(new Error('gone'));
    current.resize();
    await flush();
    expect(listener).not.toHaveBeenCalled();
    warn.mockRestore();
  });

  it('survives a failed resize registration', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    current.win.onResized.mockRejectedValueOnce(new Error('no events'));
    const unsubscribe = createTauriWindowControls().onMaximizedChange(vi.fn());
    await flush();
    expect(() => unsubscribe()).not.toThrow();
    await flush();
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});
