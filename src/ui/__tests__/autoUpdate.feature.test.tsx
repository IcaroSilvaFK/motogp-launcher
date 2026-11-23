// Binds tests/features/frontend/auto-update.feature end to end: the real launcher controller,
// the real update controller and the real UI, with only the ports replaced by in-memory fakes.
//
// The scenario "Publish the feed only through the production flow" is not a frontend concern:
// it is executed by the Rust cucumber runner (src-tauri/tests/features/updater.feature). Its
// steps here only prove that the Rust feature still states the same rules, so the two cannot
// drift apart silently.
// The real restart on the new version ("the app restarts on version 1.1.0") is proven with a
// real two-version update in issue #11; here the restart is the requested `restart()` call.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';
import { createLauncherController, createUpdateController, selectSessionClean } from '../../application';
import type { LauncherController, UpdateController } from '../../application';
import type { FakeGatewayOptions } from '../../application/__tests__/fakes';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import { FakeUpdateService } from '../../application/__tests__/fakeUpdateService';
import type { UpdateInfo } from '../../domain';
import { App } from '../App';
import { loadFrontendFeature } from './features';

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

const rustFeature = () =>
  readFileSync(`${import.meta.dirname}/../../../src-tauri/tests/features/updater.feature`, 'utf8');

const feature = await loadFrontendFeature('auto-update');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  Scenario('Find and install a newer version', ({ Given, And, When, Then }) => {
    let configure: (service: RecordingUpdateService) => void = () => {};
    let world: World;

    Given('the installed app is on version "1.0.0"', () => {
      configure = (service) => {
        service.checkResult = release('1.1.0', '1.0.0', 'Faster startup and bug fixes');
      };
    });

    And('the service reports the release "1.1.0" with notes and a valid signature', () => {
      const previous = configure;
      configure = (service) => {
        previous(service);
        service.progressEvents = [
          { kind: 'started', total: 100 },
          { kind: 'progress', chunk: 40 },
        ];
        service.holdDownload();
      };
    });

    When('the app starts', async () => {
      world = await startApp(configure);
    });

    Then('I see "New version 1.1.0 available" and the notes', async () => {
      expect(await screen.findByRole('heading', { name: 'New version 1.1.0 available' })).toBeInTheDocument();
      expect(screen.getByText('Faster startup and bug fixes')).toBeInTheDocument();
      expect(updateButton()).toBeEnabled();
    });

    When('I click Update with no pending game session', async () => {
      await world.user.click(updateButton());
    });

    Then('I see the download progress', async () => {
      const progress = await screen.findByRole('progressbar', { name: 'Downloading update' });
      expect(progress).toHaveAttribute('value', '40');
      expect(progress).toHaveAttribute('max', '100');
      expect(world.service.restartCalls).toBe(0);
    });

    And('the installation completes before the restart request', async () => {
      await act(async () => {
        world.service.releaseDownload();
      });
      await waitFor(() => expect(world.service.restartCalls).toBe(1));
      expect(world.service.order).toEqual(['download-start', 'install-done', 'restart']);
    });

    And('the app restarts on version "1.1.0"', () => {
      expect(world.service.restartCalls).toBe(1);
    });
  });

  Scenario('Already on the latest version', ({ Given, When, Then, And }) => {
    let world: World;

    Given('the installed app and the latest release are on version "1.1.0"', () => {
      // The service reports no newer release: startApp configures it in the When step.
    });

    When('the app starts', async () => {
      world = await startApp((service) => {
        service.checkResult = null;
      });
    });

    Then('no update banner is shown', () => {
      expect(world.service.checkCalls).toBe(1);
      expect(banner()).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Update' })).not.toBeInTheDocument();
    });

    And('no download or restart is requested', () => {
      expect(world.service.downloadCalls).toBe(0);
      expect(world.service.restartCalls).toBe(0);
    });
  });

  Scenario('Do not install an older version', ({ Given, When, Then }) => {
    let world: World;

    Given('the app on version "1.10.0" and a feed with "1.9.0"', () => {
      // The feed offers 1.9.0 to an app on 1.10.0: configured in the When step.
    });

    When('I check for updates', async () => {
      world = await startApp((service) => {
        service.checkResult = release('1.9.0', '1.10.0');
      });
    });

    Then('the older release is not offered for installation', () => {
      expect(world.updater.getState().kind).not.toBe('available');
      expect(banner()).not.toBeInTheDocument();
      expect(screen.queryByRole('button', { name: 'Update' })).not.toBeInTheDocument();
      expect(world.service.downloadCalls).toBe(0);
    });
  });

  Scenario('Keep the launcher usable offline', ({ Given, When, Then, And }) => {
    let world: World;

    Given('the feed query fails due to the network', () => {
      // The update service rejects the feed query: configured in the When step.
    });

    When('the app starts', async () => {
      world = await startApp((service) => {
        service.checkError = { kind: 'network', message: 'offline' };
      });
    });

    Then('I see a recoverable notice', async () => {
      expect(await screen.findByRole('alert')).toHaveTextContent('Could not reach the update server');
    });

    And('I can still use library, profiles and settings', async () => {
      await openScreen(world.user, 'Profiles');
      expect(screen.getByRole('heading', { name: 'Profiles' })).toBeInTheDocument();
      await openScreen(world.user, 'Settings');
      expect(screen.getByRole('heading', { name: 'Settings' })).toBeInTheDocument();
      await openScreen(world.user, 'Library');
      expect(screen.getByRole('heading', { name: 'Library' })).toBeInTheDocument();
    });

    And('no restart is requested', () => {
      expect(world.service.restartCalls).toBe(0);
      expect(world.service.downloadCalls).toBe(0);
    });
  });

  Scenario('Refuse a package with an invalid signature', ({ Given, When, Then, And }) => {
    let world: World;

    Given('an update whose package fails the signature verification', async () => {
      world = await startApp((service) => {
        service.checkResult = release('1.1.0', '1.0.0');
        service.installError = { kind: 'signature', message: 'bad signature' };
      });
    });

    When('I try to update it', async () => {
      await world.user.click(await screen.findByRole('button', { name: 'Update' }));
    });

    Then('the installation is refused and the error is visible', async () => {
      expect(await screen.findByRole('alert')).toHaveTextContent('failed signature verification');
      expect(world.updater.getState().kind).toBe('error');
    });

    And('the app neither restarts nor replaces the installed version', () => {
      expect(world.service.restartCalls).toBe(0);
    });
  });

  Scenario('Handle an installation failure', ({ Given, When, Then, And }) => {
    let world: World;

    Given('the download finishes but the installation fails', async () => {
      world = await startApp((service) => {
        service.checkResult = release('1.1.0', '1.0.0');
        service.installError = { kind: 'install', message: 'disk full' };
      });
    });

    When('the service returns the error', async () => {
      await world.user.click(await screen.findByRole('button', { name: 'Update' }));
    });

    Then('the error is visible and the attempt is no longer in progress', async () => {
      expect(await screen.findByRole('alert')).toHaveTextContent('could not be installed');
      expect(world.updater.getState().kind).toBe('error');
      expect(screen.queryByRole('progressbar')).not.toBeInTheDocument();
    });

    And('no restart is requested', () => {
      expect(world.service.restartCalls).toBe(0);
    });
  });

  Scenario('Avoid simultaneous installs', ({ Given, When, Then }) => {
    let world: World;

    Given('an update in progress', async () => {
      world = await startApp((service) => {
        service.checkResult = release('1.1.0', '1.0.0');
        service.progressEvents = [{ kind: 'started', total: null }];
        service.holdDownload();
      });
      await world.user.click(await screen.findByRole('button', { name: 'Update' }));
      await screen.findByRole('progressbar');
      expect(updateButton()).toBeDisabled();
    });

    When('I trigger Update again', async () => {
      await world.user.click(updateButton());
      await act(async () => {
        await world.updater.install();
      });
    });

    Then('there is no second download nor second installation', async () => {
      expect(world.service.downloadCalls).toBe(1);

      await act(async () => {
        world.service.releaseDownload();
      });
      await waitFor(() => expect(world.service.restartCalls).toBe(1));
      expect(world.service.downloadCalls).toBe(1);
    });
  });

  Scenario('Protect a game session', ({ Given, When, Then, But }) => {
    let world: World;

    Given('a session starting, running or awaiting cleanup', async () => {
      world = await startApp((service) => {
        service.checkResult = release('1.1.0', '1.0.0');
      });
      expect(updateButton()).toBeEnabled();
      act(() => world.session.emitSession({ state: 'started', cleaned: 0 }));
    });

    When('an update becomes available', async () => {
      expect(await screen.findByRole('heading', { name: 'New version 1.1.0 available' })).toBeInTheDocument();
    });

    Then('I can see the new version', () => {
      expect(banner()).toBeInTheDocument();
    });

    But('the installation and restart are unavailable until the session is clean', async () => {
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
  });

  // The "awaiting cleanup" variant of the scenario above (a crashed session whose cleanup is
  // still pending). The feature wording covers it, so it is one more check, not a new scenario.
  it('Protect a game session: cleanup pending after a crash', async () => {
    try {
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
    } finally {
      cleanup();
    }
  });

  Scenario('Publish the feed only through the production flow', ({ Given, When, Then, And }) => {
    Given('a push to an environment branch with approved gates', () => {
      expect(rustFeature()).toContain('Given the validation workflow');
    });

    When('the pipeline produces the installers', () => {
      expect(rustFeature()).toContain('the installer build needs the Rust and Frontend gates and runs only on push');
    });

    Then('dev and qas keep NSIS and MSI for 30 days with no production release', () => {
      expect(rustFeature()).toContain('dev and qas keep the installers as 30-day artifacts without a release');
    });

    And('main publishes the release of the configured version with latest.json and signed artifacts', () => {
      expect(rustFeature()).toContain('only main creates the release with the updater feed');
    });

    And('the app configuration contains only the public key', () => {
      expect(rustFeature()).toContain('the updater public key is a real minisign public key without private material');
    });
  });
});
