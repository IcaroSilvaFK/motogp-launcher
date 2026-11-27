import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { createTauriDiagnostics } from '../tauri/TauriDiagnostics';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

beforeEach(() => invokeMock.mockReset());
afterEach(() => vi.unstubAllGlobals());

describe('createTauriDiagnostics', () => {
  it('reads the snapshot with no arguments', async () => {
    invokeMock.mockResolvedValueOnce({ os: 'Windows 11', logLines: ['a', 3, 'b'] });
    expect(await createTauriDiagnostics().snapshot()).toEqual({ os: 'Windows 11', logLines: ['a', 'b'] });
    expect(invokeMock).toHaveBeenCalledWith('get_diagnostics');
  });

  it('tolerates a malformed answer', async () => {
    invokeMock.mockResolvedValueOnce({});
    expect(await createTauriDiagnostics().snapshot()).toEqual({ os: 'unknown', logLines: [] });
  });

  it('opens the logs folder without sending a path', async () => {
    invokeMock.mockResolvedValueOnce(null);
    await createTauriDiagnostics().openLogsFolder();
    expect(invokeMock).toHaveBeenCalledWith('open_logs_folder');
  });

  it('writes the text to the clipboard', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    await createTauriDiagnostics().copyText('hello');
    expect(writeText).toHaveBeenCalledWith('hello');
  });

  it('rejects when there is no clipboard', async () => {
    vi.stubGlobal('navigator', {});
    await expect(createTauriDiagnostics().copyText('x')).rejects.toThrow('clipboard');
  });
});
