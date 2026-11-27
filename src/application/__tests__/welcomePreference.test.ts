import { describe, expect, it, vi } from 'vitest';
import type { WelcomeStore } from '../ports';
import { loadWelcomeSeen, saveWelcomeSeen } from '../welcomePreference';

function storeWith(value: boolean | null): WelcomeStore {
  return { load: () => value, save: vi.fn() };
}

const broken: WelcomeStore = {
  load: () => {
    throw new Error('blocked');
  },
  save: () => {
    throw new Error('blocked');
  },
};

describe('Given the first-run welcome preference', () => {
  it('When nothing is stored Then the welcome was not seen', () => {
    expect(loadWelcomeSeen(storeWith(null))).toBe(false);
  });

  it.each([true, false])('When %j is stored Then it is returned', (seen) => {
    expect(loadWelcomeSeen(storeWith(seen))).toBe(seen);
  });

  it('When the store fails to read Then the welcome counts as seen', () => {
    expect(loadWelcomeSeen(broken)).toBe(true);
  });

  it('When saving Then the store receives true', () => {
    const store = storeWith(null);
    saveWelcomeSeen(store);
    expect(store.save).toHaveBeenCalledWith(true);
  });

  it('When the store fails to save Then no error escapes', () => {
    expect(() => saveWelcomeSeen(broken)).not.toThrow();
  });
});
