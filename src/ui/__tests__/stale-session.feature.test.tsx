// Binds tests/features/frontend/stale-session.feature end to end: the real application
// controller and the real UI, with only the ports replaced by the in-memory fakes.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
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
const OLD = 'E:/Old/MotoGP25/Content/Paks/~mods';
const stale = { mods_dir: OLD, tracked: 3 };

async function launch(options: FakeGatewayOptions = {}) {
  const gateway = new FakeGateway({
    game,
    config: { game_dir: root, active_profile: 'vanilla' },
    manifest: stale,
    ...options,
  });
  const picker = new FakePicker();
  const controller = createLauncherController({
    gateway,
    picker,
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
  });
  const user = userEvent.setup();
  render(<App controller={controller} locale="en" />);
  await waitFor(() => expect(controller.getState().status).toBe('ready'));
  await waitFor(() => expect(controller.getState().busy).toBe(false));
  return { gateway, picker, controller, user };
}

type World = Awaited<ReturnType<typeof launch>>;

const playButton = () => screen.getByRole('button', { name: 'Play' });
const noticeRegion = () => screen.queryByRole('region', { name: 'Old session pending' });
const staleNoticeGone = () => waitFor(() => expect(noticeRegion()).not.toBeInTheDocument());

const feature = await loadFrontendFeature('stale-session');

describeFeature(feature, ({ Background, Scenario, AfterEachScenario }) => {
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  // The app is launched lazily so a scenario can start from its own manifest (its own Given)
  // instead of the Background default.
  let world: World | undefined;
  const getWorld = async () => (world ??= await launch());
  const startWith = async (options: FakeGatewayOptions) => (world = await launch(options));

  Background(({ Given }) => {
    Given('the manifest points to a previous game install', () => {
      world = undefined;
    });
  });

  Scenario('The user forgets the old session', ({ When, Then, And }) => {
    When('the user chooses "Forget the old session"', async () => {
      const current = await getWorld();
      expect(playButton()).toBeDisabled();
      await current.user.click(screen.getByRole('button', { name: 'Forget the old session' }));
    });

    Then('the notice disappears', async () => {
      const current = await getWorld();
      await staleNoticeGone();
      expect(current.gateway.manifestState()).toBe('none');
      expect(await screen.findByRole('status')).toHaveTextContent('Old session forgotten');
    });

    And('Play is enabled again', async () => {
      await waitFor(() => expect(playButton()).toBeEnabled());
    });
  });

  Scenario('The user cleans the old folder', ({ When, Then, And }) => {
    When('the user confirms cleaning the old folder', async () => {
      const current = await getWorld();
      await current.user.click(screen.getByRole('button', { name: 'Clean the old folder' }));
      await current.user.click(screen.getByRole('button', { name: 'Confirm' }));
    });

    Then('only files proven to be ours are removed', async () => {
      const current = await getWorld();
      await staleNoticeGone();
      // The shown folder is the one confirmed to the backend, which removes only its own files.
      expect(current.gateway.callsOf('cleanStaleSession').map((call) => call.args)).toEqual([[OLD]]);
      expect(current.gateway.manifestState()).toBe('none');
      expect(await screen.findByRole('status')).toHaveTextContent('Old folder cleaned; 3 files removed');
    });

    And('the notice disappears', async () => {
      await staleNoticeGone();
    });
  });

  Scenario('Play is blocked while a stale session is pending', ({ Then, And }) => {
    Then('a notice names the old folder', async () => {
      await getWorld();
      const notice = screen.getByRole('region', { name: 'Old session pending' });
      expect(notice).toHaveTextContent(OLD);
      expect(notice).toHaveTextContent('3 files');
    });

    And('Play is disabled', () => {
      expect(playButton()).toBeDisabled();
    });
  });

  Scenario('Cancelling the confirmation keeps everything', ({ When, Then, And }) => {
    When('the user chooses "Clean the old folder" and cancels', async () => {
      const current = await getWorld();
      await current.user.click(screen.getByRole('button', { name: 'Clean the old folder' }));
      expect(screen.getByRole('region', { name: 'Old session pending' })).toHaveTextContent(OLD);
      await current.user.click(screen.getByRole('button', { name: 'Cancel' }));
    });

    Then('nothing is removed', async () => {
      const current = await getWorld();
      expect(current.gateway.callsOf('cleanStaleSession')).toHaveLength(0);
      expect(current.gateway.callsOf('forgetStaleSession')).toHaveLength(0);
      expect(current.gateway.manifestState()).toBe('stale');
    });

    And('the notice is still shown', () => {
      expect(noticeRegion()).toBeInTheDocument();
      expect(playButton()).toBeDisabled();
    });
  });

  Scenario('Cleaning is refused when the confirmed folder does not match', ({ When, Then, And }) => {
    When('cleaning is confirmed with a different folder', async () => {
      const current = await getWorld();
      await current.user.click(screen.getByRole('button', { name: 'Clean the old folder' }));
      // The manifest changed behind the notice, so the confirmed folder no longer matches.
      if (current.gateway.manifest) current.gateway.manifest.mods_dir = 'F:/Elsewhere/~mods';
      await current.user.click(screen.getByRole('button', { name: 'Confirm' }));
    });

    Then('the error "deploy_unauthorized_directory" is shown', async () => {
      const current = await getWorld();
      await screen.findByRole('alert');
      expect(current.controller.getState().error?.code).toBe('deploy_unauthorized_directory');
    });

    And('nothing is removed and the notice is still shown', async () => {
      const current = await getWorld();
      expect(current.gateway.manifestState()).toBe('stale');
      expect(noticeRegion()).toBeInTheDocument();
    });
  });

  Scenario('Switching the game folder makes the session stale', ({ Given, When, Then }) => {
    Given('a session deployed in the current game folder', async () => {
      const current = await startWith({ manifest: { mods_dir: game.mods_dir, tracked: 2 } });
      expect(noticeRegion()).not.toBeInTheDocument();
      current.picker.queueDirectory('D:/Steam/Other');
    });

    When('the user configures another game folder', async () => {
      const current = await getWorld();
      await current.user.click(screen.getByRole('button', { name: 'Settings' }));
      await current.user.click(screen.getByRole('button', { name: 'Choose folder…' }));
    });

    Then('the stale notice appears', async () => {
      const notice = await screen.findByRole('region', { name: 'Old session pending' });
      expect(notice).toHaveTextContent(game.mods_dir);
      expect(notice).toHaveTextContent('2 files');
    });
  });

  Scenario('A vanished old folder needs no recovery', ({ Given, Then, And }) => {
    Given('the manifest points to a folder that no longer exists', async () => {
      await startWith({ manifest: { ...stale, vanished: true } });
    });

    Then('no stale notice is shown', () => {
      expect(noticeRegion()).not.toBeInTheDocument();
    });

    And('Play is enabled', () => {
      expect(playButton()).toBeEnabled();
    });
  });

  Scenario('A stale manifest at startup unblocks Play once resolved', ({ Given, When, Then }) => {
    Given('the startup recovery failed because of the stale manifest', async () => {
      await startWith({ recovery: { state: 'failed', error: 'stale manifest', cleaned_count: 0 } });
    });

    Then('the stale notice is shown instead of the generic recovery error', () => {
      expect(noticeRegion()).toBeInTheDocument();
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      expect(playButton()).toBeDisabled();
    });

    When('the user forgets the old session', async () => {
      const current = await getWorld();
      await current.user.click(screen.getByRole('button', { name: 'Forget the old session' }));
      await staleNoticeGone();
      await waitFor(() => expect(playButton()).toBeEnabled());
    });

    Then('Play deploys normally', async () => {
      const current = await getWorld();
      await current.user.click(playButton());
      await waitFor(() => expect(current.gateway.callsOf('play').map((call) => call.args)).toEqual([['vanilla']]));
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });
  });
});
