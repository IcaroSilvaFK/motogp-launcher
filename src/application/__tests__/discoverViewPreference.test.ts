import { describe, expect, it } from 'vitest';
import { loadDiscoverView, saveDiscoverView } from '../discoverViewPreference';
import type { DiscoverView, DiscoverViewStore } from '../ports';

function fakeStore(initial: DiscoverView | null = null): DiscoverViewStore & { value: DiscoverView | null } {
  const store = {
    value: initial,
    load: () => store.value,
    save: (view: DiscoverView) => {
      store.value = view;
    },
  };
  return store;
}

const throwingStore: DiscoverViewStore = {
  load: () => {
    throw new Error('storage unavailable');
  },
  save: () => {
    throw new Error('storage unavailable');
  },
};

describe('loadDiscoverView', () => {
  it.each<DiscoverView>(['grid', 'list'])('returns the stored view %j', (view) => {
    expect(loadDiscoverView(fakeStore(view))).toBe(view);
  });

  it('starts in the grid when nothing is stored', () => {
    expect(loadDiscoverView(fakeStore(null))).toBe('grid');
  });

  it('starts in the grid when the store throws', () => {
    expect(loadDiscoverView(throwingStore)).toBe('grid');
  });
});

describe('saveDiscoverView', () => {
  it('saves the view', () => {
    const store = fakeStore();
    saveDiscoverView(store, 'list');
    expect(store.value).toBe('list');
    saveDiscoverView(store, 'grid');
    expect(store.value).toBe('grid');
  });

  it('never throws when the store fails', () => {
    expect(() => saveDiscoverView(throwingStore, 'list')).not.toThrow();
  });
});
