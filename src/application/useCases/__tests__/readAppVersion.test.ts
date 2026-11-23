import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppInfo } from '../../ports/AppInfo';
import { readAppVersion } from '../readAppVersion';

afterEach(() => vi.restoreAllMocks());

describe('readAppVersion', () => {
  it('returns the trimmed version', async () => {
    const info: AppInfo = { getVersion: async () => '  1.2.3 \n' };
    expect(await readAppVersion(info)).toBe('1.2.3');
  });

  it('returns null for an empty or whitespace version', async () => {
    expect(await readAppVersion({ getVersion: async () => '' })).toBeNull();
    expect(await readAppVersion({ getVersion: async () => '   ' })).toBeNull();
  });

  it('returns null and warns when the port rejects', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const info: AppInfo = { getVersion: () => Promise.reject(new Error('boom')) };
    expect(await readAppVersion(info)).toBeNull();
    expect(warn).toHaveBeenCalled();
  });

  it('returns null and warns when getVersion throws synchronously', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const info: AppInfo = {
      getVersion: () => {
        throw new Error('sync boom');
      },
    };
    expect(await readAppVersion(info)).toBeNull();
    expect(warn).toHaveBeenCalled();
  });
});
