import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AdViewModel } from '../../domain';
import type { AdProvider } from '../ports/AdProvider';
import { useAd } from '../useAd';

const creative: AdViewModel = {
  id: 'c1',
  title: 'Sponsor',
  body: 'Body',
  imageDataUri: 'data:image/png;base64,AAAA',
  sponsorLabel: 'Acme',
};

function provider(get: AdProvider['get'] = () => Promise.resolve(creative)) {
  return {
    get: vi.fn(get),
    click: vi.fn(() => Promise.resolve()),
    reportShown: vi.fn(() => Promise.resolve()),
  };
}

afterEach(() => vi.restoreAllMocks());

describe('useAd', () => {
  it('starts with no ad and resolves the creative of the slot', async () => {
    const ads = provider();
    const { result } = renderHook(() => useAd(ads, 'discover', true));
    expect(result.current.ad).toBeNull();
    await waitFor(() => expect(result.current.ad).toEqual(creative));
    expect(ads.get).toHaveBeenCalledWith('discover');
  });

  it('requests nothing without a provider or when disabled', () => {
    const ads = provider();
    const none = renderHook(() => useAd(undefined, 'sidebar', true));
    const off = renderHook(() => useAd(ads, 'sidebar', false));
    expect(none.result.current.ad).toBeNull();
    expect(off.result.current.ad).toBeNull();
    expect(ads.get).not.toHaveBeenCalled();
  });

  it('treats a rejection or a synchronous throw as no ad', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const rejecting = provider(() => Promise.reject(new Error('boom')));
    const throwing = provider(() => {
      throw new Error('sync');
    });
    const a = renderHook(() => useAd(rejecting, 'about', true));
    const b = renderHook(() => useAd(throwing, 'about', true));
    await waitFor(() => expect(rejecting.get).toHaveBeenCalled());
    await act(() => Promise.resolve());
    expect(a.result.current.ad).toBeNull();
    expect(b.result.current.ad).toBeNull();
  });

  it('ignores an answer that arrives after unmount', async () => {
    let resolve: (ad: AdViewModel) => void = () => undefined;
    const ads = provider(() => new Promise((done) => (resolve = done)));
    const { result, unmount } = renderHook(() => useAd(ads, 'discover', true));
    unmount();
    resolve(creative);
    await act(() => Promise.resolve());
    expect(result.current.ad).toBeNull();
  });

  it('clears the ad when it becomes disabled', async () => {
    const ads = provider();
    const { result, rerender } = renderHook(({ enabled }) => useAd(ads, 'discover', enabled), {
      initialProps: { enabled: true },
    });
    await waitFor(() => expect(result.current.ad).not.toBeNull());
    rerender({ enabled: false });
    expect(result.current.ad).toBeNull();
  });

  it('clicks with the slot and the creative id only, swallowing failures', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const ads = provider();
    ads.click.mockImplementation(() => Promise.reject(new Error('no browser')));
    const { result } = renderHook(() => useAd(ads, 'discover', true));
    await waitFor(() => expect(result.current.ad).not.toBeNull());
    act(() => result.current.click());
    await act(() => Promise.resolve());
    expect(ads.click).toHaveBeenCalledWith('discover', 'c1');
  });

  it('does nothing on click or shown while there is no ad', () => {
    const ads = provider(() => new Promise(() => undefined));
    const { result } = renderHook(() => useAd(ads, 'discover', true));
    result.current.click();
    result.current.shown();
    expect(ads.click).not.toHaveBeenCalled();
    expect(ads.reportShown).not.toHaveBeenCalled();
  });

  it('reports a creative as shown once per mount, swallowing failures', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const ads = provider();
    ads.reportShown.mockImplementation(() => Promise.reject(new Error('offline')));
    const { result } = renderHook(() => useAd(ads, 'sidebar', true));
    await waitFor(() => expect(result.current.ad).not.toBeNull());
    act(() => {
      result.current.shown();
      result.current.shown();
    });
    await act(() => Promise.resolve());
    expect(ads.reportShown).toHaveBeenCalledTimes(1);
    expect(ads.reportShown).toHaveBeenCalledWith('sidebar', 'c1');
  });

  it('swallows a synchronous throw of click and reportShown', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const ads = provider();
    ads.click.mockImplementation(() => {
      throw new Error('sync');
    });
    ads.reportShown.mockImplementation(() => {
      throw new Error('sync');
    });
    const { result } = renderHook(() => useAd(ads, 'about', true));
    await waitFor(() => expect(result.current.ad).not.toBeNull());
    expect(() => result.current.click()).not.toThrow();
    expect(() => result.current.shown()).not.toThrow();
  });
});
