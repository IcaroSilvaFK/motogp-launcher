// Binds tests/features/frontend/session-subscription.feature end to end: the real application
// controller and the real UI, with only the ports replaced by the in-memory fakes.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { expect } from 'vitest';
import { createLauncherController } from '../../application';
import type { LauncherController } from '../../application';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { GameInfo } from '../../domain';
import { App } from '../App';
import { loadFrontendFeature } from './features';

const game: GameInfo = {
  name: 'MotoGP 25',
  root: 'D:/Steam/MotoGP25',
  mods_dir: 'D:/Steam/MotoGP25/MotoGP25/Content/Paks/~mods',
  app_id: 1234,
};

const feature = await loadFrontendFeature('session-subscription');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  Scenario('The session event subscription fails', ({ Given, When, Then, And }) => {
    let controller: LauncherController;

    Given('the session event subscription rejects', () => {
      controller = createLauncherController({
        gateway: new FakeGateway({ game }),
        picker: new FakePicker(),
        session: new FakeSessionEvents({ rejectSession: new Error('listen failed') }),
        drop: new FakeDropEvents(),
      });
    });

    When('the launcher starts', async () => {
      render(<App controller={controller} locale="en" />);
      await waitFor(() => expect(controller.getState().status).toBe('ready'));
    });

    Then('an error notice is announced', async () => {
      expect(await screen.findByRole('alert')).toHaveTextContent('Session events are unavailable');
    });

    And('Play stays disabled', () => {
      expect(screen.getByRole('button', { name: 'Play' })).toBeDisabled();
      expect(screen.getByRole('status')).toHaveTextContent('Play is disabled');
    });
  });
});
