import { afterEach, describe, expect, it, vi } from 'vitest';
import { createLocalStorageLocaleStore } from '../localStorageLocaleStore';

const KEY = 'motogp-launcher.locale';

function memoryStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => void data.set(key, value),
    data,
  };
}

const throwingStorage = {
  getItem: (): string | null => {
    throw new Error('blocked');
  },
  setItem: (): void => {
    throw new Error('blocked');
  },
};

afterEach(() => {
  vi.restoreAllMocks();
  window.localStorage.clear();
});

describe('Given the localStorage locale store', () => {
  it('When saving then loading Then the locale round-trips under the expected key', () => {
    const storage = memoryStorage();
    const store = createLocalStorageLocaleStore(storage);
    store.save('en');
    expect(storage.data.get(KEY)).toBe('en');
    expect(store.load()).toBe('en');
  });

  it('When nothing is stored Then load returns null', () => {
    expect(createLocalStorageLocaleStore(memoryStorage()).load()).toBeNull();
  });

  it.each(['fr', ''])('When the stored value is %j Then load returns null', (value) => {
    expect(createLocalStorageLocaleStore(memoryStorage({ [KEY]: value })).load()).toBeNull();
  });

  it('When the storage throws Then load returns null and save propagates the error', () => {
    const store = createLocalStorageLocaleStore(throwingStorage);
    expect(store.load()).toBeNull();
    expect(() => store.save('en')).toThrow('blocked');
  });

  it('When no storage is given Then it uses window.localStorage', () => {
    const store = createLocalStorageLocaleStore();
    store.save('en');
    expect(window.localStorage.getItem(KEY)).toBe('en');
    expect(store.load()).toBe('en');
  });

  it('When reading window.localStorage throws Then creating the store does not throw, load returns null and save propagates', () => {
    vi.spyOn(window, 'localStorage', 'get').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError');
    });
    let store!: ReturnType<typeof createLocalStorageLocaleStore>;
    expect(() => {
      store = createLocalStorageLocaleStore();
    }).not.toThrow();
    expect(store.load()).toBeNull();
    expect(() => store.save('en')).toThrow();
  });
});
