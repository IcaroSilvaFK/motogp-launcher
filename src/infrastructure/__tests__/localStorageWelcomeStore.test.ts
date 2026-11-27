import { afterEach, describe, expect, it } from 'vitest';
import { createLocalStorageWelcomeStore } from '../localStorageWelcomeStore';

const KEY = 'motogp-launcher.welcome-seen';

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

describe('Given the localStorage welcome store', () => {
  it.each([
    [true, 'true'],
    [false, 'false'],
  ])('When saving %j Then it stores %j under the expected key and loads it back', (seen, raw) => {
    const storage = memoryStorage();
    const store = createLocalStorageWelcomeStore(storage);
    store.save(seen);
    expect(storage.data.get(KEY)).toBe(raw);
    expect(store.load()).toBe(seen);
  });

  it('When nothing or an invalid value is stored Then load returns null', () => {
    expect(createLocalStorageWelcomeStore(memoryStorage()).load()).toBeNull();
    expect(createLocalStorageWelcomeStore(memoryStorage({ [KEY]: 'yes' })).load()).toBeNull();
  });

  it('When the storage is blocked Then load and save throw for the application layer to handle', () => {
    const store = createLocalStorageWelcomeStore(throwingStorage);
    expect(() => store.load()).toThrow('blocked');
    expect(() => store.save(true)).toThrow('blocked');
  });

  it('When no storage is passed Then it uses window.localStorage', () => {
    createLocalStorageWelcomeStore().save(true);
    expect(window.localStorage.getItem(KEY)).toBe('true');
  });
});
