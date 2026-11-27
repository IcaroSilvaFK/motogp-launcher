import { describe, expect, it } from 'vitest';
import { createLocalStorageLibrarySortStore } from '../localStorageLibrarySortStore';

const KEY = 'motogp-launcher.library-sort';

function memoryStorage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => void data.set(key, value),
    data,
  };
}

describe('Given the localStorage Library sort store', () => {
  it.each(['name', 'size'] as const)('round-trips %s', (sort) => {
    const storage = memoryStorage();
    const store = createLocalStorageLibrarySortStore(storage);
    store.save(sort);
    expect(storage.data.get(KEY)).toBe(sort);
    expect(store.load()).toBe(sort);
  });

  it('loads null for a missing or invalid value', () => {
    expect(createLocalStorageLibrarySortStore(memoryStorage()).load()).toBeNull();
    expect(createLocalStorageLibrarySortStore(memoryStorage({ [KEY]: 'date' })).load()).toBeNull();
  });

  it('loads null when storage throws', () => {
    const throwing = {
      getItem: (): string | null => {
        throw new Error('blocked');
      },
      setItem: (): void => undefined,
    };
    expect(createLocalStorageLibrarySortStore(throwing).load()).toBeNull();
  });
});
