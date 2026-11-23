// End-to-end scenarios of tests/features/frontend/auto-update.feature: the real launcher
// controller, the real update controller and the real UI, with only the ports replaced by
// in-memory fakes.
//
// The scenario "Publish the feed only through the production flow" is not a frontend concern:
// it is covered by the Rust cucumber runner (src-tauri/tests/features/updater.feature).
// The real restart on the new version ("the app restarts on version 1.1.0") is proven with a
// real two-version update in issue #11; here the restart is the requested `restart()` call.
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import { createLauncherController, createUpdateController, selectSessionClean } from '../../application';
import type { LauncherController, UpdateController } from '../../application';
import type { FakeGatewayOptions } from '../../application/__tests__/fakes';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import { FakeUpdateService } from '../../application/__tests__/fakeUpdateService';
import type { UpdateInfo } from '../../domain';
import { App } from '../App';

afterEach(cleanup);

/** Records the order of download/install and restart requests. */
class RecordingUpdateService extends FakeUpdateService {
  readonly order: string[] = [];

  override async downloadAndInstall(onProgress: Parameters<FakeUpdateService['downloadAndInstall']>[0]) {
    this.order.push('download-start');
    await super.downloadAndInstall(onProgress);
    this.order.push('install-done');
  }

  override restart(): Promise<void> {
    this.order.push('restart');
    return super.restart();
  }
}

const release = (version: string, currentVersion: string, notes: string | null = null): UpdateInfo => ({
  version,
  currentVersion,
  notes,
  date: null,
});

interface World {
  service: RecordingUpdateService;
  launcher: LauncherController;
  updater: UpdateController;
  session: FakeSessionEvents;
  user: ReturnType<typeof userEvent.setup>;
}

/** Mounts the real UI and runs the startup check the composition root performs. */
async function startApp(
  configure: (service: RecordingUpdateService) => void,
  gatewayOptions: FakeGatewayOptions = {},
): Promise<World> {
  const service = new RecordingUpdateService();
  configure(service);
  const session = new FakeSessionEvents();
  const launcher = createLauncherController({
    gateway: new FakeGateway(gatewayOptions),
    picker: new FakePicker(),
    session,
    drop: new FakeDropEvents(),
  });
  const updater = createUpdateController({
    service,
    isSessionClean: () => selectSessionClean(launcher.getState()),
  });
  const user = userEvent.setup();
  render(<App controller={launcher} updater={updater} locale="en" />);
  await waitFor(() => expect(launcher.getState().status).toBe('ready'));
  await act(async () => {
    await updater.check();
  });
  return { service, launcher, updater, session, user };
}

const updateButton = () => screen.getByRole('button', { name: 'Update' });
const banner = () => screen.queryByRole('heading', { name: /New version/ });
const openScreen = (user: World['user'], name: string) =>
  user.click(within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', { name }));

describe('Feature: Update the installed launcher through the interface', () => {
  it('Scenario: Find and install a newer version', async () => {
    const world = await startApp((service) => {
      service.checkResult = release('1.1.0', '1.0.0', 'Faster startup and bug fixes');
      service.progressEvents = [
        { kind: 'started', total: 100 },
        { kind: 'progress', chunk: 40 },
      ];
      service.holdDownload();
    });

    expect(await screen.findByRole('heading', { name: 'New version 1.1.0 available' })).toBeInTheDocument();
    expect(screen.getByText('Faster startup and bug fixes')).toBeInTheDocument();
    expect(updateButton()).toBeEnabled();

    await world.user.click(updateButton());
    const progress = await screen.findByRole('progressbar', { name: 'Downloading update' });
    expect(progress).toHaveAttribute('value', '40');
    expect(progress).toHaveAttribute('max', '100');
    expect(world.service.restartCalls).toBe(0);

    await act(async () => {
      world.service.releaseDownload();
    });
    await waitFor(() => expect(world.service.restartCalls).toBe(1));
    expect(world.service.order).toEqual(['download-start', 'install-done', 'restart']);
  });

  it('Scenario: Already on the latest version', async () => {
    const world = await startApp((service) => {
      service.checkResult = null;
    });

    expect(world.service.checkCalls).toBe(1);
    expect(banner()).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Update' })).not.toBeInTheDocument();
    expect(world.service.downloadCalls).toBe(0);
    expect(world.service.restartCalls).toBe(0);
  });

  it('Scenario: Do not install an older version', async () => {
    const world = await startApp((service) => {
      service.checkResult = release('1.9.0', '1.10.0');
    });

    expect(world.updater.getState().kind).not.toBe('available');
    expect(banner()).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Update' })).not.toBeInTheDocument();
    expect(world.service.downloadCalls).toBe(0);
  });

  it('Scenario: Keep the launcher usable offline', async () => {
    const world = await startApp((service) => {
      service.checkError = { kind: 'network', message: 'offline' };
    });

    expect(await screen.findByRole('alert')).toHaveTextContent('Could not reach the update server');
    await openScreen(world.user, 'Profiles');
    expect(screen.getByRole('heading', { name: 'Profiles' })).toBeInTheDocument();
    await openScreen(world.user, 'Settings');
    expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
    await openScreen(world.user, 'Library');
    expect(screen.getByRole('heading', { name: 'Library' })).toBeInTheDocument();
    expect(world.service.restartCalls).toBe(0);
    expect(world.service.downloadCalls).toBe(0);
  });

  it('Scenario: Refuse a package with an invalid signature', async () => {
    const world = await startApp((service) => {
      service.checkResult = release('1.1.0', '1.0.0');
      service.installError = { kind: 'signature', message: 'bad signature' };
    });

    await world.user.click(await screen.findByRole('button', { name: 'Update' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('failed signature verification');
    expect(world.updater.getState().kind).toBe('error');
    expect(world.service.restartCalls).toBe(0);
  });

  it('Scenario: Handle an installation failure', async () => {
    const world = await startApp((service) => {
      service.checkResult = release('1.1.0', '1.0.0');
      service.installError = { kind: 'install', message: 'disk full' };
    });

    await world.user.click(await screen.findByRole('button', { name: 'Update' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('could not be installed');
    expect(world.updater.getState().kind).toBe('error');
    expect(screen.queryByRole('progressbar')).not.toBeInTheDocument();
    expect(world.service.restartCalls).toBe(0);
  });

  it('Scenario: Avoid simultaneous installs', async () => {
    const world = await startApp((service) => {
      service.checkResult = release('1.1.0', '1.0.0');
      service.progressEvents = [{ kind: 'started', total: null }];
      service.holdDownload();
    });

    await world.user.click(await screen.findByRole('button', { name: 'Update' }));
    await screen.findByRole('progressbar');
    expect(updateButton()).toBeDisabled();

    await world.user.click(updateButton());
    await act(async () => {
      await world.updater.install();
    });
    expect(world.service.downloadCalls).toBe(1);

    await act(async () => {
      world.service.releaseDownload();
    });
    await waitFor(() => expect(world.service.restartCalls).toBe(1));
    expect(world.service.downloadCalls).toBe(1);
  });

  it('Scenario: Protect a game session', async () => {
    const world = await startApp((service) => {
      service.checkResult = release('1.1.0', '1.0.0');
    });
    expect(updateButton()).toBeEnabled();

    act(() => world.session.emitSession({ state: 'started', cleaned: 0 }));

    expect(await screen.findByRole('heading', { name: 'New version 1.1.0 available' })).toBeInTheDocument();
    await waitFor(() => expect(updateButton()).toBeDisabled());
    expect(screen.getByText(/Finish the session before updating/)).toBeInTheDocument();

    // Calling install directly (bypassing the disabled button) is refused by the use case.
    await act(async () => {
      await world.updater.install();
    });
    expect(world.service.downloadCalls).toBe(0);
    expect(world.service.restartCalls).toBe(0);
    expect(world.updater.getState()).toMatchObject({ kind: 'error', error: { kind: 'session' } });
  });

  it('Scenario: Protect a game session (cleanup pending after a crash)', async () => {
    const world = await startApp(
      (service) => {
        service.checkResult = release('1.1.0', '1.0.0');
      },
      { recovery: { state: 'game-running', error: null, cleaned_count: 0 } },
    );

    expect(await screen.findByRole('heading', { name: 'New version 1.1.0 available' })).toBeInTheDocument();
    await waitFor(() => expect(updateButton()).toBeDisabled());

    await act(async () => {
      await world.updater.install();
    });
    expect(world.service.downloadCalls).toBe(0);
    expect(world.service.restartCalls).toBe(0);
  });

  // Scenario: Publish the feed only through the production flow
  // -> covered by the Rust cucumber runner (src-tauri/tests/features/updater.feature).
});
