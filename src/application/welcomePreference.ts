import type { WelcomeStore } from './ports';

/**
 * Whether the first-run welcome was already finished or skipped. Nothing stored means a first
 * launch (false). A store that throws counts as seen: a broken storage must not show the welcome
 * on every launch.
 */
export function loadWelcomeSeen(store: WelcomeStore): boolean {
  try {
    return store.load() ?? false;
  } catch {
    return true;
  }
}

/** Remembers that the welcome was finished or skipped. A storage failure must not keep it open. */
export function saveWelcomeSeen(store: WelcomeStore): void {
  try {
    store.save(true);
  } catch {
    // Ignored on purpose: the welcome still closes for this session.
  }
}
