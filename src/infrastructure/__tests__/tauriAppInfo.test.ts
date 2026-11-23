import { describe, expect, it, vi } from 'vitest';
import { getVersion } from '@tauri-apps/api/app';
import { createTauriAppInfo } from '../tauri/TauriAppInfo';

vi.mock('@tauri-apps/api/app', () => ({ getVersion: vi.fn() }));

const getVersionMock = vi.mocked(getVersion);

describe('createTauriAppInfo', () => {
  it('resolves the SDK version', async () => {
    getVersionMock.mockResolvedValueOnce('1.2.3');
    expect(await createTauriAppInfo().getVersion()).toBe('1.2.3');
  });

  it('propagates a rejection', async () => {
    getVersionMock.mockRejectedValueOnce(new Error('no app'));
    await expect(createTauriAppInfo().getVersion()).rejects.toThrow('no app');
  });
});
