// End-to-end scenarios of tests/features/frontend/stale-session.feature: the real application
// controller and the real UI, with only the ports replaced by the in-memory fakes.
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import { createLauncherController } from '../../application';
import type { FakeGatewayOptions } from '../../application/__tests__/fakes';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { GameInfo } from '../../domain';
import { App } from '../App';

afterEach(cleanup);

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

const playButton = () => screen.getByRole('button', { name: 'Play' });
const noticeRegion = () => screen.queryByRole('region', { name: 'Old session pending' });
const staleNoticeGone = () => waitFor(() => expect(noticeRegion()).not.toBeInTheDocument());

describe('Given the manifest points to a previous game install', () => {
  it('When the user chooses "Forget the old session" Then the notice disappears and Play is enabled again', async () => {
    const world = await launch();
    expect(playButton()).toBeDisabled();

    await world.user.click(screen.getByRole('button', { name: 'Forget the old session' }));

    await staleNoticeGone();
    expect(world.gateway.manifestState()).toBe('none');
    expect(await screen.findByRole('status')).toHaveTextContent('Old session forgotten');
    await waitFor(() => expect(playButton()).toBeEnabled());
  });

  it('When the user confirms cleaning the old folder Then only files proven to be ours are removed and the notice disappears', async () => {
    const world = await launch();

    await world.user.click(screen.getByRole('button', { name: 'Clean the old folder' }));
    await world.user.click(screen.getByRole('button', { name: 'Confirm' }));

    await staleNoticeGone();
    // The shown folder is the one confirmed to the backend, which removes only its own files.
    expect(world.gateway.callsOf('cleanStaleSession').map((call) => call.args)).toEqual([[OLD]]);
    expect(world.gateway.manifestState()).toBe('none');
    expect(await screen.findByRole('status')).toHaveTextContent('Old folder cleaned; 3 files removed');
  });

  it('Then a notice names the old folder and Play is disabled', async () => {
    await launch();

    const notice = screen.getByRole('region', { name: 'Old session pending' });
    expect(notice).toHaveTextContent(OLD);
    expect(notice).toHaveTextContent('3 files');
    expect(playButton()).toBeDisabled();
  });

  it('When the user chooses "Clean the old folder" and cancels Then nothing is removed and the notice is still shown', async () => {
    const world = await launch();

    await world.user.click(screen.getByRole('button', { name: 'Clean the old folder' }));
    expect(screen.getByRole('region', { name: 'Old session pending' })).toHaveTextContent(OLD);
    await world.user.click(screen.getByRole('button', { name: 'Cancel' }));

    expect(world.gateway.callsOf('cleanStaleSession')).toHaveLength(0);
    expect(world.gateway.callsOf('forgetStaleSession')).toHaveLength(0);
    expect(world.gateway.manifestState()).toBe('stale');
    expect(noticeRegion()).toBeInTheDocument();
    expect(playButton()).toBeDisabled();
  });

  it('When cleaning is confirmed with a different folder Then the error "deploy_unauthorized_directory" is shown and nothing is removed', async () => {
    const world = await launch();
    await world.user.click(screen.getByRole('button', { name: 'Clean the old folder' }));
    // The manifest changed behind the notice, so the confirmed folder no longer matches.
    if (world.gateway.manifest) world.gateway.manifest.mods_dir = 'F:/Elsewhere/~mods';

    await world.user.click(screen.getByRole('button', { name: 'Confirm' }));

    await screen.findByRole('alert');
    expect(world.controller.getState().error?.code).toBe('deploy_unauthorized_directory');
    expect(world.gateway.manifestState()).toBe('stale');
    expect(noticeRegion()).toBeInTheDocument();
  });
});

describe('Given a session deployed in the current game folder', () => {
  it('When the user configures another game folder Then the stale notice appears', async () => {
    const world = await launch({ manifest: { mods_dir: game.mods_dir, tracked: 2 } });
    expect(noticeRegion()).not.toBeInTheDocument();
    world.picker.queueDirectory('D:/Steam/Other');

    await world.user.click(screen.getByRole('button', { name: 'Settings' }));
    await world.user.click(screen.getByRole('button', { name: 'Choose folder…' }));

    const notice = await screen.findByRole('region', { name: 'Old session pending' });
    expect(notice).toHaveTextContent(game.mods_dir);
    expect(notice).toHaveTextContent('2 files');
  });
});

describe('Given the manifest points to a folder that no longer exists', () => {
  it('Then no stale notice is shown and Play is enabled', async () => {
    await launch({ manifest: { ...stale, vanished: true } });

    expect(noticeRegion()).not.toBeInTheDocument();
    expect(playButton()).toBeEnabled();
  });
});

describe('Given the startup recovery failed because of the stale manifest', () => {
  it('Then the stale notice is shown instead of the generic recovery error, and after forgetting Play deploys normally', async () => {
    const world = await launch({ recovery: { state: 'failed', error: 'stale manifest', cleaned_count: 0 } });

    expect(noticeRegion()).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(playButton()).toBeDisabled();

    await world.user.click(screen.getByRole('button', { name: 'Forget the old session' }));
    await staleNoticeGone();
    await waitFor(() => expect(playButton()).toBeEnabled());
    await world.user.click(playButton());

    await waitFor(() => expect(world.gateway.callsOf('play').map((call) => call.args)).toEqual([['vanilla']]));
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
