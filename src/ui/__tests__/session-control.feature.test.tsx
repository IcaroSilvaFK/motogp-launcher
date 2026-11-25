// Binds tests/features/frontend/session-control.feature end to end: the real application
// controller and the real UI, with only the ports replaced by the in-memory fakes.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect } from 'vitest';
import { createLauncherController } from '../../application';
import type { FakeGatewayOptions } from '../../application/__tests__/fakes';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { GameInfo } from '../../domain';
import { App } from '../App';
import { loadFrontendFeature } from './features';

const root = 'D:/Steam/MotoGP25';
const game: GameInfo = {
  name: 'MotoGP 25',
  root,
  mods_dir: `${root}/MotoGP25/Content/Paks/~mods`,
  app_id: 1234,
};
// Built from local time, so it reads 14:05 (2:05 PM in en) in whatever time zone the test runs.
const SINCE_14_05 = new Date(2026, 10, 24, 14, 5).getTime();

async function launch(options: FakeGatewayOptions = {}, before?: (gateway: FakeGateway) => void) {
  const gateway = new FakeGateway({ game, config: { game_dir: root, active_profile: 'vanilla' }, ...options });
  before?.(gateway);
  const session = new FakeSessionEvents();
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session,
    drop: new FakeDropEvents(),
  });
  const user = userEvent.setup();
  render(<App controller={controller} locale="en" />);
  await waitFor(() => expect(controller.getState().status).toBe('ready'));
  await waitFor(() => expect(controller.getState().busy).toBe(false));
  return { gateway, session, controller, user };
}

type World = Awaited<ReturnType<typeof launch>>;

const statusBar = () => screen.getByRole('contentinfo');
const actionButton = (name: string) => within(statusBar()).queryByRole('button', { name });
const sessionStatus = () => within(statusBar()).getByRole('status');

/** Starts a session through Play and lets the backend report the game as seen. */
async function runningSession(since: number | null = SINCE_14_05): Promise<World> {
  const world = await launch();
  await world.user.click(screen.getByRole('button', { name: 'Play' }));
  await waitFor(() => expect(world.controller.getState().session.kind).toBe('starting'));
  act(() => world.session.emitSession({ state: 'started', cleaned: 0, started_at: since }));
  await waitFor(() => expect(world.controller.getState().session.kind).toBe('running'));
  return world;
}

const feature = await loadFrontendFeature('session-control');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  Scenario('Play turns into Stop while the game runs', ({ Given, Then, And }) => {
    Given('the game session is running since 14:05', async () => {
      await runningSession();
    });

    Then('the status bar shows "Running since 2:05 PM"', () => {
      expect(sessionStatus()).toHaveTextContent('Running since 2:05 PM');
    });

    And('the main action is "Stop" instead of "Play"', () => {
      const stop = actionButton('Stop');
      expect(stop).toBeEnabled();
      // Destructive actions are outline danger buttons, never the filled accent.
      expect(stop).toHaveClass('btn-danger');
      expect(stop).not.toHaveClass('btn-primary');
      expect(actionButton('Play')).not.toBeInTheDocument();
    });
  });

  Scenario('Stop asks for confirmation', ({ Given, When, Then, And }) => {
    let world: World;

    Given('the game session is running', async () => {
      world = await runningSession();
    });

    When('the user clicks "Stop"', async () => {
      await world.user.click(actionButton('Stop')!);
    });

    Then('a confirmation is shown', () => {
      const confirmation = within(statusBar()).getByRole('group', { name: /Stop the game\?/ });
      expect(confirmation).toHaveTextContent('Unsaved in-game progress may be lost.');
      const confirm = within(confirmation).getByRole('button', { name: 'Stop game' });
      expect(confirm).toHaveClass('btn-danger');
      // Focus moves to the safe answer (#89): Enter or Space right after opening never stops the game.
      expect(confirm).not.toHaveFocus();
      expect(within(confirmation).getByRole('button', { name: 'Cancel' })).toHaveFocus();
    });

    And('the session is not stopped until the user confirms', async () => {
      expect(world.gateway.callsOf('stopSession')).toHaveLength(0);
      expect(world.controller.getState().session.kind).toBe('running');

      // Escape cancels and gives focus back to Stop; Cancel does the same.
      await world.user.keyboard('{Escape}');
      expect(within(statusBar()).queryByRole('group')).not.toBeInTheDocument();
      expect(actionButton('Stop')).toHaveFocus();

      await world.user.click(actionButton('Stop')!);
      await world.user.click(within(statusBar()).getByRole('button', { name: 'Cancel' }));
      expect(actionButton('Stop')).toHaveFocus();
      expect(world.gateway.callsOf('stopSession')).toHaveLength(0);
      expect(world.controller.getState().session.kind).toBe('running');
    });
  });

  Scenario('Confirming Stop stops the session and shows the cleanup', ({ Given, When, Then, And }) => {
    let world: World;

    Given('the game session is running', async () => {
      world = await runningSession();
    });

    When('the user stops the session and confirms', async () => {
      await world.user.click(actionButton('Stop')!);
      await world.user.click(within(statusBar()).getByRole('button', { name: 'Stop game' }));
    });

    Then('the backend is asked to stop the session', async () => {
      await waitFor(() => expect(world.gateway.callsOf('stopSession')).toHaveLength(1));
    });

    And('the status bar shows "Stopping the game…"', () => {
      expect(sessionStatus()).toHaveTextContent('Stopping the game…');
      const progress = actionButton('Stopping…');
      expect(progress).toHaveAttribute('aria-disabled', 'true');
      // Focus lands on the action instead of being lost with the confirmation.
      expect(progress).toHaveFocus();
      expect(actionButton('Play')).not.toBeInTheDocument();
      expect(actionButton('Stop')).not.toBeInTheDocument();
    });

    When('the backend reports "cleaning-up"', () => {
      act(() => world.session.emitSession({ state: 'cleaning-up', cleaned: 0, started_at: SINCE_14_05 }));
    });

    Then('the status bar shows "Cleaning up…"', () => {
      expect(sessionStatus()).toHaveTextContent('Cleaning up…');
      expect(actionButton('Cleaning up…')).toHaveAttribute('aria-disabled', 'true');
      expect(actionButton('Play')).not.toBeInTheDocument();
    });

    When('the backend reports the session ended with 2 files cleaned', () => {
      act(() => world.session.emitSession({ state: 'ended', cleaned: 2, started_at: SINCE_14_05 }));
    });

    Then('the main action is "Play" again', async () => {
      await waitFor(() => expect(actionButton('Play')).toBeEnabled());
      expect(actionButton('Play')).toHaveClass('btn-primary');
      expect(actionButton('Stop')).not.toBeInTheDocument();
      expect(sessionStatus()).toHaveTextContent('Session ended — 2 files removed');
    });
  });

  Scenario('A failed stop is reported and Stop stays available', ({ Given, When, Then, And }) => {
    let world: World;

    Given('the game session is running', async () => {
      world = await runningSession();
    });

    When('stopping fails with "stop_failed"', async () => {
      world.gateway.failNext('stopSession', { code: 'stop_failed', message: 'access denied' });
      await world.user.click(actionButton('Stop')!);
      await world.user.click(within(statusBar()).getByRole('button', { name: 'Stop game' }));
    });

    Then('an error explains the game could not be stopped', async () => {
      expect(await screen.findByRole('alert')).toHaveTextContent('Could not stop the game. It may still be running.');
    });

    And('the main action is still "Stop"', async () => {
      await waitFor(() => expect(actionButton('Stop')).toBeEnabled());
      expect(actionButton('Stop')).not.toHaveAttribute('aria-disabled');
      expect(sessionStatus()).toHaveTextContent('Running since 2:05 PM');
      expect(actionButton('Play')).not.toBeInTheDocument();
    });
  });

  Scenario('The session survives a page reload', ({ Given, When, Then, And }) => {
    let configure: (gateway: FakeGateway) => void;

    Given('the backend reports a running session since 14:05', () => {
      configure = (gateway) => {
        gateway.current = { phase: 'running', started_at: SINCE_14_05 };
      };
    });

    When('the launcher loads', async () => {
      await launch({}, configure);
    });

    Then('the status bar shows "Running since 2:05 PM"', async () => {
      await waitFor(() => expect(sessionStatus()).toHaveTextContent('Running since 2:05 PM'));
    });

    And('Play is not offered', () => {
      expect(actionButton('Play')).not.toBeInTheDocument();
      expect(actionButton('Stop')).toBeEnabled();
    });
  });

  Scenario('The Stop confirmation focuses Cancel', ({ Given, When, Then, And }) => {
    let world: World;

    Given('the game session is running', async () => {
      world = await runningSession();
    });

    When('the user clicks "Stop"', async () => {
      await world.user.click(actionButton('Stop')!);
    });

    Then('the focus is on "Cancel"', () => {
      const confirmation = within(statusBar()).getByRole('group', { name: /Stop the game\?/ });
      expect(within(confirmation).getByRole('button', { name: 'Cancel' })).toHaveFocus();
    });

    When('the user presses Enter', async () => {
      await world.user.keyboard('{Enter}');
    });

    Then('the session is not stopped', () => {
      expect(within(statusBar()).queryByRole('group')).not.toBeInTheDocument();
      expect(world.gateway.callsOf('stopSession')).toHaveLength(0);
      expect(world.controller.getState().session.kind).toBe('running');
    });

    And('the main action is "Stop"', () => {
      // The question closed through Cancel, so the focus is back on the main action.
      expect(actionButton('Stop')).toBeEnabled();
      expect(actionButton('Stop')).toHaveFocus();
    });
  });

  Scenario('Stop shows the busy indicator while the game is starting', ({ Given, Then, When }) => {
    let world: World;

    Given('the game is starting', async () => {
      world = await launch();
      await world.user.click(screen.getByRole('button', { name: 'Play' }));
      await waitFor(() => expect(world.controller.getState().session.kind).toBe('starting'));
    });

    Then('the main action is "Stop" and it is marked busy', async () => {
      const stop = actionButton('Stop')!;
      expect(stop).toBeEnabled();
      expect(stop).not.toHaveAttribute('aria-disabled');
      expect(stop).toHaveAttribute('aria-busy', 'true');
      expect(stop).toHaveClass('btn-danger', 'play', 'play-busy');
      // Still a real action: it opens the confirmation while starting.
      await world.user.click(stop);
      expect(within(statusBar()).getByRole('group', { name: /Stop the game\?/ })).toBeInTheDocument();
      await world.user.keyboard('{Escape}');
      expect(actionButton('Stop')).toHaveFocus();
    });

    When('the backend reports the session is running', async () => {
      act(() => world.session.emitSession({ state: 'started', cleaned: 0, started_at: SINCE_14_05 }));
      await waitFor(() => expect(world.controller.getState().session.kind).toBe('running'));
    });

    Then('the main action "Stop" is no longer marked busy', () => {
      const stop = actionButton('Stop')!;
      expect(stop).toBeEnabled();
      expect(stop).not.toHaveAttribute('aria-busy');
      expect(stop).not.toHaveClass('play-busy');
    });
  });

  Scenario('Running since follows the UI language', ({ Given, When, Then }) => {
    let world: World;

    /** Picks a language in Settings, the way the user does. */
    async function chooseLanguage(locale: string) {
      const nav = screen.getByRole('navigation', { name: /Main navigation|Navegação principal/ });
      await world.user.click(within(nav).getByRole('button', { name: /^(Settings|Configurações)$/ }));
      await world.user.selectOptions(screen.getByRole('combobox', { name: /Language|Idioma/ }), locale);
    }

    Given('the game session is running since 14:05', async () => {
      world = await runningSession();
    });

    When('the language is "en"', async () => {
      await chooseLanguage('en');
    });

    // toHaveTextContent collapses whitespace (U+202F included), so the narrow no-break space some
    // ICU versions put before "PM" still matches a plain space.
    Then('the status bar shows "Running since 2:05 PM"', () => {
      expect(sessionStatus()).toHaveTextContent('Running since 2:05 PM');
    });

    When('the language is "pt-BR"', async () => {
      await chooseLanguage('pt-BR');
    });

    Then('the status bar shows the Portuguese "running since" text with "14:05"', () => {
      expect(sessionStatus()).toHaveTextContent('Em execução desde 14:05');
      expect(sessionStatus()).not.toHaveTextContent(/PM|AM/);
    });
  });
});
