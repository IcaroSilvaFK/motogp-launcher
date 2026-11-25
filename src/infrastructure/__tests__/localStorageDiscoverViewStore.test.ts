import { afterEach, describe, expect, it, vi } from 'vitest';
import { createLocalStorageDiscoverViewStore } from '../localStorageDiscoverViewStore';

const KEY = 'motogp-launcher.discover-view';

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

describe('Given the localStorage Discover view store', () => {
  it.each(['grid', 'list'] as const)(
    'When saving %j Then it stores it under the expected key and loads it back',
    (view) => {
      const storage = memoryStorage();
      const store = createLocalStorageDiscoverViewStore(storage);
      store.save(view);
      expect(storage.data.get(KEY)).toBe(view);
      expect(store.load()).toBe(view);
    },
  );

  it('When nothing is stored Then load returns null', () => {
    expect(createLocalStorageDiscoverViewStore(memoryStorage()).load()).toBeNull();
  });

  it.each(['table', '', 'GRID', 'true'])('When the stored value is %j Then load returns null', (value) => {
    expect(createLocalStorageDiscoverViewStore(memoryStorage({ [KEY]: value })).load()).toBeNull();
  });

  it('When the storage throws Then load returns null and save propagates the error', () => {
    const store = createLocalStorageDiscoverViewStore(throwingStorage);
    expect(store.load()).toBeNull();
    expect(() => store.save('list')).toThrow('blocked');
  });

  it('When no storage is given Then it uses window.localStorage', () => {
    const store = createLocalStorageDiscoverViewStore();
    store.save('list');
    expect(window.localStorage.getItem(KEY)).toBe('list');
    expect(store.load()).toBe('list');
  });

  it('When reading window.localStorage throws Then creating the store does not throw, load returns null and save propagates', () => {
    vi.spyOn(window, 'localStorage', 'get').mockImplementation(() => {
      throw new DOMException('blocked', 'SecurityError');
    });
    let store!: ReturnType<typeof createLocalStorageDiscoverViewStore>;
    expect(() => {
      store = createLocalStorageDiscoverViewStore();
    }).not.toThrow();
    expect(store.load()).toBeNull();
    expect(() => store.save('grid')).toThrow();
  });
});
