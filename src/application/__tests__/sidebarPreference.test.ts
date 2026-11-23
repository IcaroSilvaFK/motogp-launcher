import { describe, expect, it } from 'vitest';
import type { NavPreferenceStore } from '../ports';
import { loadSidebarCollapsed, saveSidebarCollapsed } from '../sidebarPreference';

function fakeStore(initial: boolean | null = null): NavPreferenceStore & { value: boolean | null } {
  const store = {
    value: initial,
    load: () => store.value,
    save: (collapsed: boolean) => {
      store.value = collapsed;
    },
  };
  return store;
}

const throwingStore: NavPreferenceStore = {
  load: () => {
    throw new Error('storage unavailable');
  },
  save: () => {
    throw new Error('storage unavailable');
  },
};

describe('loadSidebarCollapsed', () => {
  it.each([true, false])('returns the stored choice %j', (value) => {
    expect(loadSidebarCollapsed(fakeStore(value))).toBe(value);
  });

  it('starts expanded when nothing is stored', () => {
    expect(loadSidebarCollapsed(fakeStore(null))).toBe(false);
  });

  it('starts expanded when the store throws', () => {
    expect(loadSidebarCollapsed(throwingStore)).toBe(false);
  });
});

describe('saveSidebarCollapsed', () => {
  it('saves the choice', () => {
    const store = fakeStore();
    saveSidebarCollapsed(store, true);
    expect(store.value).toBe(true);
    saveSidebarCollapsed(store, false);
    expect(store.value).toBe(false);
  });

  it('never throws when the store fails', () => {
    expect(() => saveSidebarCollapsed(throwingStore, true)).not.toThrow();
  });
});
