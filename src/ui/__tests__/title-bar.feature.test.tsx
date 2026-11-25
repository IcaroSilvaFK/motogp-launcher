// Binds tests/features/frontend/title-bar.feature: the real App over a fake controller and a fake
// WindowControls port, so the wiring is the one main.tsx uses.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import type { WindowControls } from '../../application';
import type { Locale } from '../i18n';
import { App } from '../App';
import { createFakeController } from './fakeController';
import { loadFrontendFeature } from './features';

type FakeWindowControls = WindowControls & {
  [K in 'minimize' | 'toggleMaximize' | 'close' | 'isMaximized']: ReturnType<typeof vi.fn>;
} & { report: (maximized: boolean) => void };

function fakeWindowControls(): FakeWindowControls {
  const listeners = new Set<(maximized: boolean) => void>();
  return {
    minimize: vi.fn(() => Promise.resolve()),
    toggleMaximize: vi.fn(() => Promise.resolve()),
    close: vi.fn(() => Promise.resolve()),
    isMaximized: vi.fn(() => Promise.resolve(false)),
    onMaximizedChange: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    report: (maximized) => listeners.forEach((listener) => listener(maximized)),
  };
}

function rejectingWindowControls(): WindowControls {
  const fail = () => Promise.reject(new Error('no window'));
  return {
    minimize: fail,
    toggleMaximize: fail,
    close: fail,
    isMaximized: fail,
    onMaximizedChange: () => {
      throw new Error('no events');
    },
  };
}

let controls: FakeWindowControls;
let pending: WindowControls | undefined;
let user: ReturnType<typeof userEvent.setup>;

function launch(windowControls: WindowControls | undefined, locale: Locale = 'en') {
  user = userEvent.setup();
  render(<App controller={createFakeController()} locale={locale} windowControls={windowControls} />);
}

const titleBar = () => document.querySelector<HTMLElement>('.titlebar') as HTMLElement;
const group = (name = 'Window controls') => within(screen.getByRole('group', { name }));

function expectButtons(groupName: string, names: string[]) {
  const buttons = group(groupName).getAllByRole('button');
  expect(buttons.map((button) => button.getAttribute('aria-label') ?? button.textContent)).toEqual(names);
  for (const [index, button] of buttons.entries()) {
    expect(button).toHaveAttribute('type', 'button');
    expect(button).toHaveAttribute('title', names[index]);
  }
}

const feature = await loadFrontendFeature('title-bar');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  const openWithControls = () => {
    controls = fakeWindowControls();
    launch(controls);
  };

  Scenario('The title bar shows the app name and the window controls', ({ Given, Then, And }) => {
    Given('the launcher is open with window controls', openWithControls);
    Then('the title bar shows "MotoGP Launcher"', () => {
      expect(titleBar()).not.toBeNull();
      expect(within(titleBar()).getByText('MotoGP Launcher')).toHaveClass('titlebar-title');
      // The title bar is not a heading: the single h1 stays in the sidebar.
      expect(within(titleBar()).queryByRole('heading')).not.toBeInTheDocument();
      expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
    });
    And('the window controls group has the buttons "Minimize", "Maximize" and "Close"', () => {
      expectButtons('Window controls', ['Minimize', 'Maximize', 'Close']);
    });
  });

  Scenario('Minimizing and closing the window', ({ Given, When, Then }) => {
    Given('the launcher is open with window controls', openWithControls);
    When('I press "Minimize"', async () => {
      await user.click(group().getByRole('button', { name: 'Minimize' }));
    });
    Then('the window is asked to minimize', () => {
      expect(controls.minimize).toHaveBeenCalledTimes(1);
    });
    When('I press "Close"', async () => {
      await user.click(group().getByRole('button', { name: 'Close' }));
    });
    Then('the window is asked to close', () => {
      expect(controls.close).toHaveBeenCalledTimes(1);
      expect(controls.toggleMaximize).not.toHaveBeenCalled();
    });
  });

  Scenario('Maximize turns into Restore', ({ Given, When, Then }) => {
    Given('the launcher is open with window controls', openWithControls);
    When('I press "Maximize"', async () => {
      await user.click(group().getByRole('button', { name: 'Maximize' }));
    });
    Then('the window is asked to toggle maximize', () => {
      expect(controls.toggleMaximize).toHaveBeenCalledTimes(1);
      expect(controls.isMaximized).toHaveBeenCalled();
    });
    When('the window reports it is maximized', () => {
      act(() => controls.report(true));
    });
    Then('the window controls show "Restore" instead of "Maximize"', () => {
      expect(group().getByRole('button', { name: 'Restore' })).toHaveAttribute('title', 'Restore');
      expect(group().queryByRole('button', { name: 'Maximize' })).not.toBeInTheDocument();
    });
  });

  Scenario('The title area is the window drag region', ({ Given, Then, And }) => {
    Given('the launcher is open with window controls', openWithControls);
    Then('the title area is a drag region', () => {
      // Tauri's drag region starts the drag and maximizes on a native double-click.
      const drag = titleBar().querySelector<HTMLElement>('.titlebar-drag') as HTMLElement;
      expect(drag).toHaveAttribute('data-tauri-drag-region');
      for (const child of drag.querySelectorAll('*')) {
        if (child.closest('svg') && child.tagName.toLowerCase() !== 'svg') continue;
        expect(child, child.className.toString()).toHaveAttribute('data-tauri-drag-region');
      }
      expect(within(drag).getByText('MotoGP Launcher')).toHaveAttribute('data-tauri-drag-region');
    });
    And('no window control button is a drag region', () => {
      for (const button of group().getAllByRole('button')) {
        expect(button.closest('[data-tauri-drag-region]')).toBeNull();
        expect(button.querySelector('[data-tauri-drag-region]')).toBeNull();
      }
    });
  });

  Scenario('The window controls speak the chosen language', ({ Given, Then }) => {
    Given('the launcher is open with window controls in Portuguese', () => {
      controls = fakeWindowControls();
      launch(controls, 'pt-BR');
    });
    Then('the window controls group has the buttons "Minimizar", "Maximizar" and "Fechar"', () => {
      expectButtons('Controles da janela', ['Minimizar', 'Maximizar', 'Fechar']);
    });
  });

  Scenario('A failing window call does not break the launcher', ({ Given, When, And, Then }) => {
    Given('window controls that reject every call', () => {
      vi.spyOn(console, 'warn').mockImplementation(() => undefined);
      pending = rejectingWindowControls();
    });
    When('the launcher opens', () => {
      launch(pending);
    });
    And('I press "Maximize"', async () => {
      await user.click(group().getByRole('button', { name: 'Maximize' }));
    });
    Then('the launcher is still shown', () => {
      expect(screen.getByRole('heading', { level: 1, name: 'MotoGP Launcher' })).toBeInTheDocument();
      expect(group().getByRole('button', { name: 'Maximize' })).toBeInTheDocument();
      expect(screen.getByRole('main')).toBeInTheDocument();
    });
  });

  Scenario('Without window controls the title bar shows only the title', ({ Given, Then, And }) => {
    Given('the launcher is open without window controls', () => {
      launch(undefined);
    });
    Then('the title bar shows "MotoGP Launcher"', () => {
      expect(within(titleBar()).getByText('MotoGP Launcher')).toBeInTheDocument();
    });
    And('there is no window controls group', () => {
      expect(screen.queryByRole('group', { name: 'Window controls' })).not.toBeInTheDocument();
      expect(within(titleBar()).queryByRole('button')).not.toBeInTheDocument();
    });
  });
});
