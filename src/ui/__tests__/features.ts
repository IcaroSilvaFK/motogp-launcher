import { loadFeature } from '@amiceli/vitest-cucumber';

/**
 * Loads a frontend Gherkin file from tests/features/frontend. The path is resolved from this
 * file's directory (not from the working directory), so it works in worktrees and in CI on any OS.
 */
export function loadFrontendFeature(name: string) {
  return loadFeature(`${import.meta.dirname}/../../../tests/features/frontend/${name}.feature`);
}
