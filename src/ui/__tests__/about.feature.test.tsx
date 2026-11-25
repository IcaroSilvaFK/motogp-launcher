// Binds tests/features/frontend/about.feature end to end: the real launcher controller, the real
// update controller and the real UI, with only the ports replaced by in-memory fakes (the
// UpdateService fake and an inline AppInfo fake).
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import type { MockInstance } from 'vitest';
import { createLauncherController, createUpdateController, selectSessionClean } from '../../application';
import type { AppInfo, LauncherController, UpdateController } from '../../application';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import { FakeUpdateService } from '../../application/__tests__/fakeUpdateService';
import type { Locale, UpdateInfo } from '../../domain';
import { App } from '../App';
import { loadFrontendFeature } from './features';

/** An UpdateService whose `check()` can be held open, to observe the "checking" state. */
class GatedUpdateService extends FakeUpdateService {
  private checkGate: Promise<void> | null = null;
  private openCheck: (() => void) | null = null;

  holdCheck(): void {
    this.checkGate = new Promise<void>((resolve) => {
      this.openCheck = resolve;
    });
  }

  releaseCheck(): void {
    this.openCheck?.();
    this.checkGate = null;
    this.openCheck = null;
  }

  override async check(): Promise<UpdateInfo | null> {
    if (this.checkGate) await this.checkGate;
    return super.check();
  }
}

const release = (version: string): UpdateInfo => ({ version, currentVersion: '0.1.0', notes: null, date: null });

const game = { name: 'MotoGP 25', root: 'D:/Steam/MotoGP25', mods_dir: 'D:/Steam/MotoGP25/~mods', app_id: 1234 };
const profile = { id: 'p1', name: 'Mine', mod_ids: [] };

const RAW_ERROR = 'Could not fetch a valid release JSON from the remote';

interface Setup {
  /** What the AppInfo port returns; an Error makes it reject. */
  version: string | Error;
  locale: Locale;
  configure: (service: GatedUpdateService) => void;
  /** Runs the silent startup check the composition root performs. */
  backgroundCheck: boolean;
}

interface World {
  service: GatedUpdateService;
  launcher: LauncherController;
  updater: UpdateController;
  session: FakeSessionEvents;
  user: ReturnType<typeof userEvent.setup>;
}

let warnSpy: MockInstance<typeof console.warn> | null = null;

async function launch(overrides: Partial<Setup> = {}): Promise<World> {
  const setup: Setup = { version: '0.1.0', locale: 'en', configure: () => {}, backgroundCheck: false, ...overrides };
  warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  const service = new GatedUpdateService();
  setup.configure(service);
  const session = new FakeSessionEvents();
  const launcher = createLauncherController({
    gateway: new FakeGateway({
      game,
      profiles: [profile],
      config: { game_dir: game.root, active_profile: 'p1' },
    }),
    picker: new FakePicker(),
    session,
    drop: new FakeDropEvents(),
  });
  const updater = createUpdateController({
    service,
    isSessionClean: () => selectSessionClean(launcher.getState()),
  });
  const { version } = setup;
  const appInfo: AppInfo = {
    getVersion: () => (version instanceof Error ? Promise.reject(version) : Promise.resolve(version)),
  };
  const user = userEvent.setup();
  render(<App controller={launcher} updater={updater} appInfo={appInfo} locale={setup.locale} />);
  await waitFor(() => expect(launcher.getState().status).toBe('ready'));
  if (setup.backgroundCheck) {
    await act(async () => {
      await updater.check({ background: true });
    });
  }
  return { service, launcher, updater, session, user };
}

const statusBar = () => within(screen.getByRole('contentinfo'));
// pt-BR fixture: the Portuguese UI labels are accepted on purpose (real users run the pt-BR UI).
const mainNav = () => within(screen.getByRole('navigation', { name: /Main navigation|Navegação principal/ }));

async function openAbout(world: World, labels = { settings: 'Settings', about: 'About' }) {
  await world.user.click(mainNav().getByRole('button', { name: labels.settings }));
  const sections = within(screen.getByRole('navigation', { name: /Settings sections|Seções das configurações/ }));
  await world.user.click(sections.getByRole('link', { name: labels.about }));
}

const about = (name = 'About') => within(screen.getByRole('region', { name }));

const feature = await loadFrontendFeature('about');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(() => {
    cleanup();
    warnSpy?.mockRestore();
    warnSpy = null;
  });

  // Not a feature scenario: tests and dev builds mount the App without an updater.
  it('About shows the version and no update check without an updater', async () => {
    try {
      const launcher = createLauncherController({
        gateway: new FakeGateway({ game }),
        picker: new FakePicker(),
        session: new FakeSessionEvents(),
        drop: new FakeDropEvents(),
      });
      const user = userEvent.setup();
      render(<App controller={launcher} appInfo={{ getVersion: () => Promise.resolve(' 0.1.0 ') }} locale="en" />);
      await waitFor(() => expect(launcher.getState().status).toBe('ready'));
      await user.click(mainNav().getByRole('button', { name: 'Settings' }));
      expect(await about().findByText('0.1.0')).toBeInTheDocument();
      expect(about().queryByRole('button', { name: 'Check for updates' })).not.toBeInTheDocument();
      expect(about().queryByRole('status')).not.toBeInTheDocument();
    } finally {
      cleanup();
    }
  });

  Scenario('The status bar shows the launcher version', ({ Given, When, Then }) => {
    Given('the launcher version is "0.1.0"', () => {
      // The default launch reports version 0.1.0.
    });

    When('the launcher opens', async () => {
      await launch();
    });

    Then('the status bar shows "v0.1.0" next to the game', async () => {
      const caption = await statusBar().findByText('v0.1.0');
      const gameBlock = caption.closest('.statusbar-game') as HTMLElement;
      expect(gameBlock).not.toBeNull();
      expect(within(gameBlock).getByText('MotoGP 25')).toBeInTheDocument();
    });
  });

  Scenario('Settings About shows the launcher version', ({ Given, When, Then }) => {
    let world: World;

    Given('the launcher version is "0.1.0"', async () => {
      world = await launch();
    });

    When('the user opens Settings and the About section', async () => {
      await openAbout(world);
      const sections = within(screen.getByRole('navigation', { name: 'Settings sections' }));
      expect(sections.getByRole('link', { name: 'About' })).toHaveAttribute('aria-current', 'location');
    });

    Then('the About section shows the version "0.1.0"', async () => {
      const row = (await about().findByText('0.1.0')).closest('.setting-row') as HTMLElement;
      expect(row).not.toBeNull();
      expect(within(row).getByText('Launcher version')).toBeInTheDocument();
    });
  });

  Scenario('The version stays hidden when it cannot be read', ({ Given, When, Then, And }) => {
    let world: World;
    let failure: Error;

    Given('reading the launcher version fails', () => {
      failure = new Error('version unavailable');
    });

    When('the launcher opens', async () => {
      world = await launch({ version: failure });
      await waitFor(() => expect(warnSpy).toHaveBeenCalled());
    });

    Then('the status bar shows no version', () => {
      expect(statusBar().queryByText(/^v\d/)).not.toBeInTheDocument();
      expect(screen.queryByText('version unavailable', { exact: false })).not.toBeInTheDocument();
    });

    And('the launcher stays usable', async () => {
      expect(statusBar().getByRole('button', { name: 'Play' })).toBeEnabled();
      await openAbout(world);
      expect(about().getByRole('button', { name: 'Check for updates' })).toBeEnabled();
      await world.user.click(mainNav().getByRole('button', { name: 'Library' }));
      expect(screen.getByRole('heading', { name: 'Library' })).toBeInTheDocument();
    });
  });

  Scenario('Checking manually when already up to date', ({ Given, When, Then, And }) => {
    let world: World;

    Given('the launcher version is "0.1.0" and no newer release exists', async () => {
      world = await launch({
        configure: (service) => {
          service.checkResult = null;
          service.holdCheck();
        },
      });
      await openAbout(world);
    });

    When('the user clicks "Check for updates" in About', async () => {
      await world.user.click(about().getByRole('button', { name: 'Check for updates' }));
    });

    Then('the button is disabled and reads "Checking…" while the check runs', async () => {
      expect(about().getByRole('button', { name: 'Checking…' })).toBeDisabled();
      expect(about().queryByText('The launcher is up to date.')).not.toBeInTheDocument();
      await act(async () => {
        world.service.releaseCheck();
      });
    });

    And('About says the launcher is up to date', async () => {
      expect(await about().findByText('The launcher is up to date.')).toBeInTheDocument();
      expect(about().getByRole('status')).toHaveTextContent('The launcher is up to date.');
      expect(about().getByRole('button', { name: 'Check for updates' })).toBeEnabled();
      expect(world.service.checkCalls).toBe(1);
    });
  });

  Scenario('Checking manually finds a newer version', ({ Given, When, Then, And }) => {
    let world: World;

    Given('a release "0.2.0" is published', async () => {
      world = await launch({
        configure: (service) => {
          service.checkResult = release('0.2.0');
        },
      });
      await openAbout(world);
    });

    When('the user clicks "Check for updates" in About', async () => {
      await world.user.click(about().getByRole('button', { name: 'Check for updates' }));
    });

    Then('About says version "0.2.0" is available', async () => {
      expect(await about().findByText('Version 0.2.0 is available')).toBeInTheDocument();
      expect(about().getByRole('status')).toHaveTextContent('Version 0.2.0 is available');
    });

    And('an install action is offered that installs through the update controller', async () => {
      const install = about().getByRole('button', { name: 'Install update' });
      expect(install).toBeEnabled();
      expect(install).toHaveClass('btn-accent-outline');
      expect(install).not.toHaveClass('btn-primary');
      await world.user.click(install);
      await waitFor(() => expect(world.service.restartCalls).toBe(1));
      expect(world.service.downloadCalls).toBe(1);
    });
  });

  Scenario('A manual check that fails shows a translated error', ({ Given, When, Then, And }) => {
    let configure: (service: GatedUpdateService) => void = () => {};
    let world: World;

    Given('the update server cannot be reached', () => {
      configure = (service) => {
        service.checkError = { kind: 'network', message: RAW_ERROR };
      };
    });

    And('the language is Portuguese (Brazil)', async () => {
      // pt-BR fixture: the Portuguese UI labels below are the expected text for this locale.
      world = await launch({ configure, locale: 'pt-BR' });
      await openAbout(world, { settings: 'Configurações', about: 'Sobre' });
    });

    When('the user clicks "Check for updates" in About', async () => {
      await world.user.click(about('Sobre').getByRole('button', { name: 'Verificar atualizações' }));
    });

    Then('About shows the translated network error', async () => {
      expect(
        await about('Sobre').findByText('Não foi possível acessar o servidor de atualização', { exact: false }),
      ).toBeInTheDocument();
      expect(about('Sobre').queryByText('O launcher está atualizado.')).not.toBeInTheDocument();
    });

    And('the raw error message is not shown', () => {
      expect(screen.queryByText(RAW_ERROR, { exact: false })).not.toBeInTheDocument();
    });
  });

  Scenario('A failed background check does not claim the launcher is up to date', ({ Given, When, Then }) => {
    let world: World;

    Given('the startup check failed silently', async () => {
      world = await launch({
        configure: (service) => {
          service.checkError = { kind: 'network', message: RAW_ERROR };
        },
        backgroundCheck: true,
      });
      expect(world.updater.getState().kind).toBe('none');
    });

    When('the user opens the About section', async () => {
      await openAbout(world);
    });

    Then('About does not say the launcher is up to date', () => {
      expect(about().queryByText('The launcher is up to date.')).not.toBeInTheDocument();
      expect(about().queryByText(/Could not reach/)).not.toBeInTheDocument();
      expect(about().getByRole('button', { name: 'Check for updates' })).toBeEnabled();
    });
  });

  Scenario('Install from About is blocked during a game session', ({ Given, And, Then }) => {
    let world: World;

    Given('a release "0.2.0" is available', async () => {
      world = await launch({
        configure: (service) => {
          service.checkResult = release('0.2.0');
        },
        backgroundCheck: true,
      });
      expect(world.updater.getState().kind).toBe('available');
    });

    And('a game session is active', async () => {
      act(() => world.session.emitSession({ state: 'started', cleaned: 0, started_at: null }));
      await openAbout(world);
    });

    Then('the install action in About is disabled with the session hint', async () => {
      const install = about().getByRole('button', { name: 'Install update' });
      await waitFor(() => expect(install).toBeDisabled());
      expect(install).toHaveAccessibleDescription(/Finish the session before updating/);
      expect(about().getByText('Version 0.2.0 is available')).toBeInTheDocument();
      expect(world.service.downloadCalls).toBe(0);
    });
  });
});
