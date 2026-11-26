import { afterEach, describe, expect, it } from 'vitest';
import { createLocalStorageUsagePreferenceStore } from '../localStorageUsagePreferenceStore';

const KEY = 'motogp-launcher.share-usage';

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

describe('Given the localStorage usage preference store', () => {
  it.each(['on', 'off'] as const)('loads the valid value %j', (value) => {
    expect(createLocalStorageUsagePreferenceStore(memoryStorage({ [KEY]: value })).load()).toBe(value);
  });

  it.each(['maybe', 'true', ''])('loads null for the invalid value %j', (value) => {
    expect(createLocalStorageUsagePreferenceStore(memoryStorage({ [KEY]: value })).load()).toBeNull();
  });

  it('loads null when nothing is stored', () => {
    expect(createLocalStorageUsagePreferenceStore(memoryStorage()).load()).toBeNull();
  });

  it('lets read and write errors propagate', () => {
    const store = createLocalStorageUsagePreferenceStore(throwingStorage);
    expect(() => store.load()).toThrow('blocked');
    expect(() => store.save('off')).toThrow('blocked');
  });

  it('saves under its key', () => {
    const storage = memoryStorage();
    createLocalStorageUsagePreferenceStore(storage).save('off');
    expect(storage.data.get(KEY)).toBe('off');
  });

  it('uses window.localStorage by default', () => {
    const store = createLocalStorageUsagePreferenceStore();
    store.save('off');
    expect(window.localStorage.getItem(KEY)).toBe('off');
    expect(store.load()).toBe('off');
  });
});
