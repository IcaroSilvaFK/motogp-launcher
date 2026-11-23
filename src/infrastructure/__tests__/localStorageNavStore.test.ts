import { afterEach, describe, expect, it, vi } from 'vitest';
import { createLocalStorageNavStore } from '../localStorageNavStore';

const KEY = 'motogp-launcher.sidebar-collapsed';

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

describe('Given the localStorage sidebar preference store', () => {
  it.each([
    [true, 'true'],
    [false, 'false'],
  ])('When saving %j Then it stores %j under the expected key and loads it back', (collapsed, raw) => {
    const storage = memoryStorage();
    const store = createLocalStorageNavStore(storage);
    store.save(collapsed);
    expect(storage.data.get(KEY)).toBe(raw);
    expect(store.load()).toBe(collapsed);
  });

  it('When nothing is stored Then load returns null', () => {
    expect(createLocalStorageNavStore(memoryStorage()).load()).toBeNull();
  });

  it.each(['yes', '', 'TRUE', '1'])('When the stored value is %j Then load returns null', (value) => {
    expect(createLocalStorageNavStore(memoryStorage({ [KEY]: value })).load()).toBeNull();
  });

  it('When the storage throws Then load returns null and save propagates the error', () => {
    const store = createLocalStorageNavStore(throwingStorage);
    expect(store.load()).toBeNull();
    expect(() => store.save(true)).toThrow('blocked');
  });

  it('When no storage is given Then it uses window.localStorage', () => {
    const store = createLocalStorageNavStore();
    store.save(true);
    expect(window.localStorage.getItem(KEY)).toBe('true');
    expect(store.load()).toBe(true);
  });

  it('When reading window.localStorage throws Then creating the store does not throw, load returns null and save propagates', () => {
    vi.spyOn(window, 'localStorage', 'get').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError');
    });
    let store!: ReturnType<typeof createLocalStorageNavStore>;
    expect(() => {
      store = createLocalStorageNavStore();
    }).not.toThrow();
    expect(store.load()).toBeNull();
    expect(() => store.save(true)).toThrow();
  });
});
