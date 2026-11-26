import { describe, expect, it, vi } from 'vitest';
import type { UsagePreferenceStore } from '../ports';
import { createUsagePreference, loadUsagePreference, saveUsagePreference } from '../usagePreference';

function store(initial: 'on' | 'off' | null, { loadThrows = false, saveThrows = false } = {}) {
  const memory: { value: 'on' | 'off' | null } = { value: initial };
  return Object.assign(memory, {
    load: vi.fn((): 'on' | 'off' | null => {
      if (loadThrows) throw new Error('blocked');
      return memory.value;
    }),
    save: vi.fn((value: 'on' | 'off') => {
      if (saveThrows) throw new Error('blocked');
      memory.value = value;
    }),
  }) satisfies UsagePreferenceStore;
}

describe('loadUsagePreference', () => {
  it('is on when nothing is stored', () => {
    expect(loadUsagePreference(store(null))).toBe('on');
  });

  it('returns the stored choice', () => {
    expect(loadUsagePreference(store('off'))).toBe('off');
    expect(loadUsagePreference(store('on'))).toBe('on');
  });

  it('is off when the store throws', () => {
    expect(loadUsagePreference(store('on', { loadThrows: true }))).toBe('off');
  });
});

describe('saveUsagePreference', () => {
  it('persists the choice', () => {
    const memory = store(null);
    saveUsagePreference(memory, 'off');
    expect(memory.value).toBe('off');
  });

  it('swallows a storage failure', () => {
    expect(() => saveUsagePreference(store(null, { saveThrows: true }), 'off')).not.toThrow();
  });
});

describe('createUsagePreference', () => {
  it('reads the store once and keeps the choice in memory', () => {
    const memory = store('off');
    const preference = createUsagePreference(memory);
    expect(preference.get()).toBe('off');
    expect(preference.get()).toBe('off');
    expect(memory.load).toHaveBeenCalledTimes(1);
  });

  it('keeps an opt-out for the session even when it cannot be saved', () => {
    const memory = store('on', { saveThrows: true });
    const preference = createUsagePreference(memory);
    preference.set('off');
    expect(preference.get()).toBe('off');
    expect(memory.save).toHaveBeenCalledWith('off');
  });
});
