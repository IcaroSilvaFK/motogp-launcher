// Binds tests/features/frontend/shell-scroll.feature (#141): the shell owns scrolling, so neither
// out-of-flow content nor the Settings section jump may scroll the document.
import { readFileSync } from 'node:fs';
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import { App } from '../App';
import { createFakeController } from './fakeController';
import { loadFrontendFeature } from './features';

/** The stylesheet without comments. */
const css = readFileSync(`${import.meta.dirname}/../styles.css`, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');

/** The bodies of every rule whose selector list contains exactly `selector`, joined. */
function rule(selector: string): string {
  return [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .filter(([, list = '']) => list.split(',').some((item) => item.trim() === selector))
    .map(([, , body = '']) => body)
    .join('\n');
}

let user: ReturnType<typeof userEvent.setup>;
let scrollTo: ReturnType<typeof vi.fn>;
let intoView: ReturnType<typeof vi.fn>;
let windowScroll: ReturnType<typeof vi.spyOn>;

const feature = await loadFrontendFeature('shell-scroll');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(() => {
    cleanup();
    vi.restoreAllMocks();
    // Drop the scrollTop stub so jsdom's own accessor on Element applies again.
    delete (HTMLElement.prototype as { scrollTop?: number }).scrollTop;
  });

  Scenario('The content area is the containing block of out-of-flow content', ({ Given, Then, And }) => {
    Given('the launcher stylesheet', () => {
      expect(css.length).toBeGreaterThan(0);
    });

    // A visually hidden status (position: absolute) deep in a long page would otherwise take the
    // viewport as its containing block and stretch the document past the 100vh shell.
    Then('the content area clips absolutely positioned descendants', () => {
      const main = rule('main');
      expect(main).toMatch(/position:\s*relative/);
      expect(main).toMatch(/overflow-y:\s*auto/);
      expect(main).toMatch(/min-height:\s*0/);
    });

    And('the document itself does not scroll', () => {
      expect(rule('body')).toMatch(/overflow:\s*hidden/);
      expect(rule('.app-shell')).toMatch(/height:\s*100vh/);
    });
  });

  Scenario('Jumping to a settings section scrolls only the content area', ({ Given, When, Then, And }) => {
    Given('the launcher shows Settings', async () => {
      user = userEvent.setup();
      scrollTo = vi.fn();
      intoView = vi.fn();
      // jsdom does not scroll; record the writes and calls to tell the scroller apart from the document.
      Object.defineProperty(HTMLElement.prototype, 'scrollTop', {
        configurable: true,
        get: () => 0,
        set(this: HTMLElement, value: number) {
          scrollTo.call(this, { top: value });
        },
      });
      Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', { configurable: true, value: intoView });
      windowScroll = vi.spyOn(window, 'scrollTo').mockImplementation(() => undefined);
      render(<App controller={createFakeController()} locale="en" />);
      const nav = within(screen.getByRole('navigation', { name: 'Main navigation' }));
      await user.click(nav.getByRole('button', { name: 'Settings' }));
    });

    When('the user picks the "Storage" section', async () => {
      const sections = within(screen.getByRole('navigation', { name: 'Settings sections' }));
      await user.click(sections.getByRole('link', { name: 'Storage' }));
    });

    Then('the content area scrolls to the Storage section', () => {
      expect(scrollTo).toHaveBeenCalledTimes(1);
      expect(scrollTo.mock.contexts[0]).toBe(document.querySelector('main'));
      expect(scrollTo.mock.calls[0]?.[0]).toMatchObject({ top: expect.any(Number) });
      expect(screen.getByRole('link', { name: 'Storage' })).toHaveAttribute('aria-current', 'location');
    });

    And('the document is not scrolled', () => {
      // scrollIntoView scrolls every scrollable ancestor, the document included.
      expect(intoView).not.toHaveBeenCalled();
      expect(windowScroll).not.toHaveBeenCalled();
    });
  });
});
