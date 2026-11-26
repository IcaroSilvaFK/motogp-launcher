import { afterEach, describe, expect, it } from 'vitest';
import { createLocalStorageAdPreferenceStore } from '../localStorageAdPreferenceStore';

const KEY = 'motogp-launcher.ad-preference';

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
  window.localStorage.clear();
});

describe('Given the localStorage ad preference store', () => {
  it.each(['on', 'off'] as const)('loads the valid value %j', (value) => {
    expect(createLocalStorageAdPreferenceStore(memoryStorage({ [KEY]: value })).load()).toBe(value);
  });

  it('loads null for an invalid value', () => {
    expect(createLocalStorageAdPreferenceStore(memoryStorage({ [KEY]: 'maybe' })).load()).toBeNull();
  });

  it('loads null when nothing is stored', () => {
    expect(createLocalStorageAdPreferenceStore(memoryStorage()).load()).toBeNull();
  });

  it('lets a read error propagate', () => {
    expect(() => createLocalStorageAdPreferenceStore(throwingStorage).load()).toThrow('blocked');
  });

  it('saves under its key', () => {
    const storage = memoryStorage();
    createLocalStorageAdPreferenceStore(storage).save('off');
    expect(storage.data.get(KEY)).toBe('off');
  });

  it('uses window.localStorage by default', () => {
    const store = createLocalStorageAdPreferenceStore();
    store.save('on');
    expect(window.localStorage.getItem(KEY)).toBe('on');
    expect(store.load()).toBe('on');
  });
});
