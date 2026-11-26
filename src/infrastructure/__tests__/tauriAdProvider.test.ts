import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { createTauriAdProvider } from '../tauri/TauriAdProvider';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

const creative = {
  id: 'c1',
  title: 'Sponsor',
  body: 'Body',
  imageDataUri: 'data:image/png;base64,AAAA',
  sponsorLabel: 'Acme',
};

beforeEach(() => {
  invokeMock.mockReset();
  vi.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterEach(() => vi.restoreAllMocks());

describe('createTauriAdProvider', () => {
  it('asks get_ad for the slot and returns the creative', async () => {
    invokeMock.mockResolvedValueOnce(creative);
    expect(await createTauriAdProvider().get('discover')).toEqual(creative);
    expect(invokeMock).toHaveBeenCalledWith('get_ad', { slot: 'discover' });
  });

  it('maps a null answer to no ad', async () => {
    invokeMock.mockResolvedValueOnce(null);
    expect(await createTauriAdProvider().get('sidebar')).toBeNull();
  });

  it('maps a malformed payload to no ad', async () => {
    invokeMock.mockResolvedValueOnce({ id: 'c1', title: 'No image' });
    expect(await createTauriAdProvider().get('about')).toBeNull();
  });

  it('maps a missing command or any rejection to no ad', async () => {
    invokeMock.mockRejectedValueOnce('command get_ad not found');
    expect(await createTauriAdProvider().get('discover')).toBeNull();
    invokeMock.mockRejectedValueOnce({ code: 'internal', message: 'boom' });
    expect(await createTauriAdProvider().get('discover')).toBeNull();
  });

  it('maps a synchronous throw (no Tauri runtime) to no ad', async () => {
    invokeMock.mockImplementationOnce(() => {
      throw new TypeError('window.__TAURI_INTERNALS__ is undefined');
    });
    expect(await createTauriAdProvider().get('discover')).toBeNull();
  });

  it('sends ad_clicked and ad_shown with the slot and the creative id only', async () => {
    invokeMock.mockResolvedValue(undefined);
    const ads = createTauriAdProvider();
    await ads.click('discover', 'c1');
    await ads.reportShown('sidebar', 'c2');
    expect(invokeMock).toHaveBeenNthCalledWith(1, 'ad_clicked', { slot: 'discover', creativeId: 'c1' });
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'ad_shown', { slot: 'sidebar', creativeId: 'c2' });
  });

  it('swallows click and shown failures', async () => {
    invokeMock.mockRejectedValue('command ad_clicked not found');
    const ads = createTauriAdProvider();
    await expect(ads.click('discover', 'c1')).resolves.toBeUndefined();
    await expect(ads.reportShown('discover', 'c1')).resolves.toBeUndefined();
    invokeMock.mockImplementation(() => {
      throw new Error('sync');
    });
    await expect(ads.click('about', 'c1')).resolves.toBeUndefined();
    expect(console.warn).toHaveBeenCalled();
  });
});
