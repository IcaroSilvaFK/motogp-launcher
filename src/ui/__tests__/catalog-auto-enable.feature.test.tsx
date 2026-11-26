// Binds tests/features/frontend/catalog-auto-enable.feature (#129): the real controller over fake
// gateways, with the install button and the toast rendered together.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import { createLauncherController, useLauncherState } from '../../application';
import type { AutoEnablePreferenceStore, LauncherController } from '../../application';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import { AutoEnablePreferenceProvider } from '../AutoEnablePreferenceContext';
import { en } from '../i18n/en';
import { I18nProvider } from '../i18n';
import { CatalogInstallButton } from '../InstallButton';
import { LauncherContext } from '../LauncherContext';
import { NoticeBar } from '../NoticeBar';
import { loadFrontendFeature } from './features';

const feature = await loadFrontendFeature('catalog-auto-enable');
const ID = 'rebalanced-ai';

function Harness({ controller, onOpenLibrary }: { controller: LauncherController; onOpenLibrary: () => void }) {
  const state = useLauncherState(controller);
  return (
    <LauncherContext.Provider value={{ controller, state }}>
      <NoticeBar onOpenLibrary={onOpenLibrary} />
      <CatalogInstallButton entryId={ID} />
    </LauncherContext.Provider>
  );
}

async function launch(options: { autoEnable?: boolean } = {}) {
  const gateway = new FakeGateway({
    config: { game_dir: null, active_profile: 'default' },
    profiles: [
      { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
      { id: 'default', name: 'Default', mod_ids: [] },
    ],
  });
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
  });
  await controller.start();
  const store: AutoEnablePreferenceStore = { load: () => options.autoEnable ?? true, save: () => undefined };
  const onOpenLibrary = vi.fn();
  render(
    <I18nProvider locale="en">
      <AutoEnablePreferenceProvider store={store}>
        <Harness controller={controller} onOpenLibrary={onOpenLibrary} />
      </AutoEnablePreferenceProvider>
    </I18nProvider>,
  );
  return { gateway, controller, onOpenLibrary };
}

type World = Awaited<ReturnType<typeof launch>>;

const defaultMods = (world: World) => world.controller.getState().profiles.find((p) => p.id === 'default')!.mod_ids;

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  Scenario('Install leaves the mod ready to play', ({ Given, When, Then, And }) => {
    let world: World;
    Given('the active profile is "Default"', async () => {
      world = await launch();
    });
    When('the user installs a catalog mod', async () => {
      await userEvent.click(screen.getByRole('button', { name: 'Install' }));
    });
    Then('the mod is in the library and enabled in "Default"', async () => {
      expect(await screen.findByText(en.catalogInstall.installedEnabled)).toBeInTheDocument();
      expect(world.gateway.callsOf('installCatalogEntry')[0]?.args).toEqual([ID, true]);
      expect(world.controller.getState().mods.map((m) => m.id)).toEqual([ID]);
      expect(defaultMods(world)).toEqual([ID]);
    });
    And('the install button shows "Installed and enabled"', () => {
      expect(screen.getByText(en.catalogInstall.installedEnabled)).toBeInTheDocument();
    });
    And('the toast offers "Undo" and "Open in Library"', async () => {
      expect(screen.getByText(en.notices.modInstalledEnabled(ID))).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Undo' })).toBeInTheDocument();
      await userEvent.click(screen.getByRole('button', { name: 'Open in Library' }));
      expect(world.onOpenLibrary).toHaveBeenCalledTimes(1);
    });
  });

  Scenario('A conflicting mod stays disabled', ({ Given, When, Then, And }) => {
    let world: World;
    Given('an enabled mod replaces the same file', async () => {
      world = await launch();
      world.gateway.installConflictWith = 'Existing Pack';
    });
    When('the user installs a catalog mod that replaces it too', async () => {
      await userEvent.click(screen.getByRole('button', { name: 'Install' }));
    });
    Then('the new mod is installed but disabled', async () => {
      expect(await screen.findByText(en.catalogInstall.installed)).toBeInTheDocument();
      expect(world.controller.getState().mods.map((m) => m.id)).toEqual([ID]);
      expect(defaultMods(world)).toEqual([]);
      expect(screen.queryByRole('button', { name: 'Undo' })).not.toBeInTheDocument();
    });
    And('the user is told which mod conflicts', () => {
      expect(screen.getByText(en.notices.modInstalledConflict(ID, 'Existing Pack'))).toBeInTheDocument();
    });
  });

  Scenario('Auto-enable turned off', ({ Given, When, Then }) => {
    let world: World;
    Given('"Enable mods after installing from Discover" is off', async () => {
      world = await launch({ autoEnable: false });
    });
    When('the user installs a catalog mod', async () => {
      await userEvent.click(screen.getByRole('button', { name: 'Install' }));
    });
    Then('it is installed but not enabled', async () => {
      expect(await screen.findByText(en.catalogInstall.installed)).toBeInTheDocument();
      expect(world.gateway.callsOf('installCatalogEntry')[0]?.args).toEqual([ID, false]);
      expect(defaultMods(world)).toEqual([]);
    });
  });

  Scenario('Undo disables the mod again', ({ Given, When, And, Then }) => {
    let world: World;
    Given('the active profile is "Default"', async () => {
      world = await launch();
    });
    When('the user installs a catalog mod', async () => {
      await userEvent.click(screen.getByRole('button', { name: 'Install' }));
    });
    And('the user clicks "Undo" in the toast', async () => {
      await userEvent.click(await screen.findByRole('button', { name: 'Undo' }));
    });
    Then('the mod stays in the library but is no longer enabled in "Default"', async () => {
      await vi.waitFor(() => expect(defaultMods(world)).toEqual([]));
      expect(world.controller.getState().mods.map((m) => m.id)).toEqual([ID]);
    });
  });
});
