// Binds tests/features/frontend/motion.feature: static checks of the motion tokens and rules read
// from styles.css, plus the enter/exit state the UI exposes through data-state.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { readFileSync } from 'node:fs';
import { expect, vi } from 'vitest';
import type { LauncherState } from '../../application';
import type { ModEntry } from '../../domain';
import { App } from '../App';
import { STAGGER_CAP } from '../motion';
import { createFakeController } from './fakeController';
import type { FakeController } from './fakeController';
import { loadFrontendFeature } from './features';

const css = readFileSync(`${import.meta.dirname}/../styles.css`, 'utf8');

const rootBlock = /:root\s*\{([^}]*)\}/.exec(css)?.[1] ?? '';

/** Custom properties declared in the first :root block. */
function rootTokens(): Map<string, string> {
  const tokens = new Map<string, string>();
  for (const [, name = '', value = ''] of rootBlock.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g))
    tokens.set(name, value.trim());
  return tokens;
}

/** The stylesheet without the :root token block and without comments. */
const rulesOnly = css.replace(/\/\*[\s\S]*?\*\//g, '').replace(/:root\s*\{[^}]*\}/, '');

/** The body of the first rule whose selector list is exactly `selector`. */
function rule(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp(`(^|\\n)${escaped}\\s*\\{([^}]*)\\}`).exec(rulesOnly)?.[2] ?? '';
}

/** Every @keyframes block as [name, body]. */
function keyframes(): [string, string][] {
  return [...rulesOnly.matchAll(/@keyframes\s+([\w-]+)\s*\{((?:[^{}]*\{[^}]*\})*\s*)\}/g)].map(
    ([, name = '', body = '']) => [name, body],
  );
}

function mod(n: number): ModEntry {
  return { id: `m${n}`, name: `Mod ${n}`, files: [`m${n}.pak`], size_bytes: 1024 * n };
}

const manyMods = Array.from({ length: STAGGER_CAP + 4 }, (_, i) => mod(i + 1));

let controller: FakeController;
let user: ReturnType<typeof userEvent.setup>;

function launch(patch: Partial<LauncherState> = {}) {
  controller = createFakeController(patch);
  user = userEvent.setup();
  render(<App controller={controller} locale="en" />);
}

function preferReducedMotion(reduce: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn((query: string) => ({
      matches: reduce && query.includes('prefers-reduced-motion'),
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    })),
  );
}

const noticeText = () => screen.queryByText('Profile saved');
const notice = () => noticeText()?.closest('.notice') ?? null;
const libraryRows = () => within(screen.getByRole('list', { name: 'Library' })).getAllByRole('listitem');

function showNotice() {
  launch({ notice: { kind: 'profile-saved' } });
  vi.mocked(controller.dismissNotice).mockImplementation(() => controller.setState({ notice: null }));
  expect(notice()).toBeInTheDocument();
}

const feature = await loadFrontendFeature('motion');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  Scenario('The stylesheet defines Material motion tokens', ({ Given, Then, And }) => {
    Given('the launcher stylesheet', () => {
      expect(rootBlock.length).toBeGreaterThan(0);
    });
    Then('it defines the standard, emphasized decelerate and emphasized accelerate easings', () => {
      const tokens = rootTokens();
      expect(tokens.get('--motion-easing-standard')).toBe('cubic-bezier(0.2, 0, 0, 1)');
      expect(tokens.get('--motion-easing-emphasized-decelerate')).toBe('cubic-bezier(0.05, 0.7, 0.1, 1)');
      expect(tokens.get('--motion-easing-emphasized-accelerate')).toBe('cubic-bezier(0.3, 0, 0.8, 0.15)');
    });
    And('it defines short and medium durations', () => {
      const tokens = rootTokens();
      expect(tokens.get('--motion-duration-short2')).toBe('100ms');
      expect(tokens.get('--motion-duration-short3')).toBe('150ms');
      expect(tokens.get('--motion-duration-short4')).toBe('200ms');
      expect(tokens.get('--motion-duration-medium1')).toBe('250ms');
      expect(tokens.get('--motion-duration-medium2')).toBe('300ms');
    });
    And('it defines a stagger step and an enter distance', () => {
      const tokens = rootTokens();
      expect(tokens.get('--motion-stagger-step')).toMatch(/^\d+ms$/);
      expect(tokens.get('--motion-distance-enter')).toMatch(/^\d+px$/);
    });
  });

  Scenario('Animations only use motion tokens', ({ Given, Then, And }) => {
    Given('the launcher stylesheet', () => {
      expect(rulesOnly.length).toBeGreaterThan(0);
    });
    Then('every transition and animation outside the root block references motion tokens', () => {
      const declarations = [
        ...rulesOnly.matchAll(/(?:^|[;{\s])((?:transition|animation)(?:-[a-z-]+)?)\s*:\s*([^;}]+)/g),
      ];
      expect(declarations.length).toBeGreaterThan(10);
      for (const [, property = '', value = ''] of declarations) {
        const label = `${property}: ${value.trim()}`;
        expect(value, label).not.toMatch(/cubic-bezier|steps\(|\b\d*\.?\d+m?s\b/);
        if (!/^\s*none\b/.test(value)) expect(value, label).toMatch(/var\(--/);
      }
    });
    And('no keyframe animates a property other than transform or opacity', () => {
      const frames = keyframes();
      expect(frames.length).toBeGreaterThan(0);
      for (const [name, body] of frames) {
        for (const [, property = ''] of body.matchAll(/([\w-]+)\s*:/g)) {
          expect(['transform', 'opacity'], `@keyframes ${name}`).toContain(property);
        }
      }
    });
  });

  Scenario('Reduced motion disables non-essential motion', ({ Given, Then }) => {
    Given('the launcher stylesheet', () => {
      expect(css.length).toBeGreaterThan(0);
    });
    Then('a reduced-motion media query turns off every animation and transition', () => {
      const block = /@media\s*\(prefers-reduced-motion:\s*reduce\)\s*\{([\s\S]*?\})\s*\}/.exec(css)?.[1] ?? '';
      expect(block).toMatch(/\*,\s*\*::before,\s*\*::after/);
      expect(block).toMatch(/animation:\s*none\s*!important/);
      expect(block).toMatch(/transition:\s*none\s*!important/);
    });
  });

  Scenario('Switching screens plays an enter transition', ({ Given, When, Then }) => {
    let libraryPage: Element | null = null;
    Given('the launcher is loaded on the Library screen', () => {
      launch();
      libraryPage = screen.getByRole('heading', { level: 2, name: 'Library' }).closest('[data-motion="screen"]');
      expect(libraryPage).toBeInTheDocument();
    });
    When('the user opens the Profiles screen', async () => {
      const nav = within(screen.getByRole('navigation', { name: 'Main navigation' }));
      await user.click(nav.getByRole('button', { name: 'Profiles' }));
    });
    Then('the new page is marked as entering', () => {
      const page = screen.getByRole('heading', { level: 2, name: 'Profiles' }).closest('[data-motion="screen"]');
      expect(page).toHaveAttribute('data-state', 'entering');
      // A fresh element: the enter animation plays on mount, so a reused node would not animate.
      expect(page).not.toBe(libraryPage);
      expect(libraryPage).not.toBeInTheDocument();
      // Only the wrapper's own animation ends the enter; a row finishing its stagger does not.
      fireEvent.animationEnd(screen.getByRole('heading', { level: 2, name: 'Profiles' }));
      expect(page).toHaveAttribute('data-state', 'entering');
      fireEvent.animationEnd(page as Element);
      expect(page).toHaveAttribute('data-state', 'entered');
    });
  });

  Scenario('A notice slides out before it is removed', ({ Given, When, Then, And }) => {
    Given('a notice is shown', () => {
      preferReducedMotion(false);
      showNotice();
      expect(notice()).toHaveAttribute('data-state', 'entering');
    });
    When('the notice is dismissed', async () => {
      await user.click(screen.getByRole('button', { name: 'Dismiss' }));
    });
    Then('the notice is marked as exiting', () => {
      expect(notice()).toHaveAttribute('data-state', 'exiting');
      // Inert while it leaves: no second click, no focus, out of the accessibility tree.
      expect(notice()).toHaveAttribute('inert');
      expect(screen.queryByRole('status')).not.toBeInTheDocument();
    });
    And('it is removed after the exit duration', async () => {
      await waitFor(() => expect(noticeText()).not.toBeInTheDocument());
    });
  });

  Scenario('Under reduced motion a notice is removed at once', ({ Given, And, When, Then }) => {
    Given('the user prefers reduced motion', () => {
      preferReducedMotion(true);
    });
    And('a notice is shown', () => {
      showNotice();
    });
    When('the notice is dismissed', async () => {
      await user.click(screen.getByRole('button', { name: 'Dismiss' }));
    });
    Then('the notice is removed immediately', () => {
      expect(noticeText()).not.toBeInTheDocument();
    });
  });

  Scenario('List entrance is staggered with a cap', ({ Given, Then, And }) => {
    Given('the library has more mods than the stagger cap', () => {
      launch({ mods: manyMods });
      expect(libraryRows().length).toBeGreaterThan(STAGGER_CAP);
    });
    Then('each row carries its stagger index', () => {
      libraryRows().forEach((row, index) => {
        expect(row.style.getPropertyValue('--stagger-index')).toBe(String(Math.min(index, STAGGER_CAP)));
      });
    });
    And('no row is delayed beyond the cap', () => {
      const indexes = libraryRows().map((row) => Number(row.style.getPropertyValue('--stagger-index')));
      expect(Math.max(...indexes)).toBe(STAGGER_CAP);
      expect(rule('.stagger-item')).toMatch(
        /animation-delay:\s*calc\(var\(--stagger-index,\s*0\)\s*\*\s*var\(--motion-stagger-step\)\)/,
      );
    });
  });

  Scenario('The list entrance does not replay on re-renders', ({ Given, When, Then }) => {
    let before: HTMLElement[] = [];
    Given('the library has more mods than the stagger cap', () => {
      launch({
        mods: manyMods,
        profiles: [{ id: 'mine', name: 'Mine', mod_ids: [] }],
        config: { game_dir: null, active_profile: 'mine' },
      });
      before = libraryRows();
    });
    When('the user toggles a mod in the active profile', async () => {
      await user.click(screen.getByRole('switch', { name: 'Include Mod 1 in Mine' }));
      act(() => controller.setState({ profiles: [{ id: 'mine', name: 'Mine', mod_ids: ['m1'] }] }));
      expect(screen.getByRole('switch', { name: 'Include Mod 1 in Mine' })).toHaveAttribute('aria-checked', 'true');
    });
    Then('the rows stay mounted, so their entrance does not play again', () => {
      const after = libraryRows();
      expect(after).toHaveLength(before.length);
      after.forEach((row, index) => expect(row).toBe(before[index]));
    });
  });

  Scenario('The session dot pulses only while a session is live', ({ Given, Then, And }) => {
    Given('the launcher stylesheet', () => {
      expect(rulesOnly.length).toBeGreaterThan(0);
    });
    Then('the starting and running session dots have a pulse animation', () => {
      expect(rule('.session-status-running::before')).toMatch(/animation:\s*pulse\b/);
      expect(rule('.session-status-starting::before')).toMatch(/animation:\s*pulse\b/);
      const pulse = keyframes().find(([name]) => name === 'pulse')?.[1] ?? '';
      expect(pulse).toMatch(/opacity/);
      expect(pulse).toMatch(/transform:\s*scale/);
    });
    And('the stopping and cleaning-up session dots pulse too', () => {
      expect(rule('.session-status-stopping::before,\n.session-status-cleaning-up::before')).toMatch(
        /animation:\s*pulse\b/,
      );
    });
    And('the idle session status has none', () => {
      expect(rule('.session-status::before')).not.toMatch(/animation/);
      expect(rule('.statusbar-idle::before')).not.toMatch(/animation/);
      expect(rulesOnly).not.toMatch(/\.session-status-idle[^{]*\{[^}]*animation/);
    });
  });

  Scenario('The session action looks busy while the game stops or cleans up', ({ Given, Then, And }) => {
    Given('the launcher stylesheet', () => {
      expect(rulesOnly.length).toBeGreaterThan(0);
    });
    Then('the stopping and cleaning-up progress button shows a spinner and a progress cursor', () => {
      const spinner = rule('.play-progress::after');
      expect(spinner).toMatch(/content:\s*''/);
      expect(spinner).toMatch(/border-radius:\s*50%/);
      expect(spinner).toMatch(/animation:\s*spin\b/);
      const spin = keyframes().find(([name]) => name === 'spin')?.[1] ?? '';
      expect(spin).toMatch(/transform:\s*rotate/);
      expect(rule('.play-progress,\n.play-progress:hover:not(:disabled)')).toMatch(/cursor:\s*progress/);
    });
    And('the progress button gives no hover or press feedback', () => {
      // It is aria-disabled, not disabled, so the `:not(:disabled)` hover and press rules still match it.
      expect(rule('.play-progress,\n.play-progress:hover:not(:disabled)')).toMatch(/background:\s*transparent/);
      expect(rule('.play-progress:active:not(:disabled)')).toMatch(/transform:\s*none/);
    });
    And('Stop stays a plain action while the game is starting', () => {
      // Under #75 Play is not rendered while starting (Stop takes its slot), so no rule may key a busy
      // look off the starting status: Stop must read as clickable, the pulsing dot and the text say "starting".
      expect(rulesOnly).not.toMatch(/session-status-starting\)?[^{,]*\.play/);
      expect(rulesOnly).not.toMatch(/\.btn-danger\.play[^{]*::after/);
    });
  });
});
