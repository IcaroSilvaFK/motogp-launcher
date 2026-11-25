// Binds tests/features/frontend/catalog-install.feature (#74): the real controller over fake
// gateways, with the "install button" steps asserted against the rendered CatalogInstallButton.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect } from 'vitest';
import { createLauncherController, useLauncherState } from '../../application';
import type { LauncherController } from '../../application';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import { en } from '../i18n/en';
import { I18nProvider } from '../i18n';
import { CatalogInstallButton } from '../InstallButton';
import { LauncherContext } from '../LauncherContext';
import { loadFrontendFeature } from './features';

const feature = await loadFrontendFeature('catalog-install');
const ID = 'rebalanced-ai';

function Harness({ controller }: { controller: LauncherController }) {
  const state = useLauncherState(controller);
  return (
    <LauncherContext.Provider value={{ controller, state }}>
      <CatalogInstallButton entryId={ID} />
    </LauncherContext.Provider>
  );
}

async function launch() {
  const gateway = new FakeGateway();
  const session = new FakeSessionEvents();
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session,
    drop: new FakeDropEvents(),
  });
  await controller.start();
  render(
    <I18nProvider locale="en">
      <Harness controller={controller} />
    </I18nProvider>,
  );
  return { gateway, session, controller };
}

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  Scenario('Download progress is shown', ({ Given, Then }) => {
    let release: () => void = () => undefined;
    Given('an install of "rebalanced-ai" reports 42% downloaded', async () => {
      const world = await launch();
      world.gateway.installGate = new Promise<void>((resolve) => (release = resolve));
      await act(async () => {
        void world.controller.installCatalogEntry(ID);
        world.gateway.emitProgress({ id: ID, phase: 'downloading', received: 420, total: 1000 });
        await Promise.resolve();
      });
    });
    Then('the install button shows "42%"', async () => {
      const bar = await screen.findByRole('progressbar', { name: en.catalogInstall.progressLabel });
      expect(bar).toHaveAttribute('aria-valuenow', '42');
      expect(screen.getByText('Downloading 42%')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();
      release();
    });
  });

  Scenario('Installed entries appear in the library', ({ When, Then, And }) => {
    let world: Awaited<ReturnType<typeof launch>>;
    When('the install of "rebalanced-ai" finishes', async () => {
      world = await launch();
      await userEvent.click(screen.getByRole('button', { name: 'Install' }));
    });
    Then('the install button shows "Installed"', async () => {
      expect(await screen.findByRole('status')).toHaveTextContent('Installed');
      expect(screen.queryByRole('button')).not.toBeInTheDocument();
    });
    And('the library is reloaded', () => {
      expect(world.gateway.callsOf('listMods').length).toBeGreaterThanOrEqual(2);
      expect(world.controller.getState().mods.map((mod) => mod.id)).toEqual([ID]);
    });
  });

  Scenario('A failed install can be retried', ({ Given, Then, When }) => {
    let world: Awaited<ReturnType<typeof launch>>;
    Given('the install of "rebalanced-ai" failed with a hash mismatch', async () => {
      world = await launch();
      world.gateway.failNext('installCatalogEntry', { code: 'download_hash_mismatch', message: 'sha mismatch' });
      await userEvent.click(screen.getByRole('button', { name: 'Install' }));
    });
    Then('the translated hash mismatch message is shown', async () => {
      expect(await screen.findByRole('alert')).toHaveTextContent(en.errors.download_hash_mismatch);
    });
    When('the user clicks "Retry"', async () => {
      await userEvent.click(screen.getByRole('button', { name: 'Retry' }));
    });
    Then('the install starts again', async () => {
      expect(world.gateway.callsOf('installCatalogEntry')).toHaveLength(2);
      await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Installed'));
    });
  });

  Scenario('Install is disabled during a session', ({ Given, Then }) => {
    let world: Awaited<ReturnType<typeof launch>>;
    Given('a game session is running', async () => {
      world = await launch();
      act(() => world.session.emitSession({ state: 'started', cleaned: 0, started_at: 1 }));
    });
    Then('the install button is disabled', async () => {
      const button = screen.getByRole('button', { name: 'Install' });
      expect(button).toHaveAttribute('aria-disabled', 'true');
      expect(button).toHaveAccessibleDescription(en.errors.library_locked_by_session);
      await userEvent.click(button);
      expect(world.gateway.callsOf('installCatalogEntry')).toHaveLength(0);
    });
  });
});
