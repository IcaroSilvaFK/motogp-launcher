// Binds tests/features/frontend/session-locks.feature end to end: the real application
// controller and the real UI, with only the ports replaced by the in-memory fakes.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect } from 'vitest';
import { createLauncherController } from '../../application';
import type { FakeGatewayOptions } from '../../application/__tests__/fakes';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { ModEntry } from '../../domain';
import { App } from '../App';
import { ptBR } from '../i18n/pt-BR';
import { loadFrontendFeature } from './features';

const fast: ModEntry = { id: 'fast', name: 'Fast Bike', files: ['fast.pak'], size_bytes: 2048 };
const rain: ModEntry = { id: 'rain', name: 'Rain Tyres', files: ['rain.pak'], size_bytes: 1024 };

const REMOVE_LOCKED = 'Mods cannot be removed from the library while the game is running.';
const PROFILES_LOCKED = 'Profile changes are locked while the game is running.';

async function launch(options: FakeGatewayOptions) {
  const gateway = new FakeGateway(options);
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

const removeButton = (modName: string) => screen.getByRole('button', { name: `Remove ${modName}` });
const profileActions = () => [
  screen.getByRole('button', { name: 'Create profile' }),
  screen.getByRole('button', { name: 'Rename' }),
  screen.getByRole('button', { name: 'Delete Mine' }),
  screen.getByRole('button', { name: 'Save' }),
];
const WRITES = ['createProfile', 'renameProfile', 'deleteProfile', 'setProfileMods', 'removeMod'] as const;

const feature = await loadFrontendFeature('session-locks');

describeFeature(feature, ({ Background, Scenario, AfterEachScenario }) => {
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  // The app is launched lazily so a scenario's own Given can adjust the Background's data.
  let options: FakeGatewayOptions = {};
  let world: World | undefined;
  const getWorld = async () => (world ??= await launch(options));

  Background(({ Given, And }) => {
    Given('the library has the mods "Fast Bike" and "Rain Tyres"', () => {
      world = undefined;
      options = { mods: [fast, rain] };
    });
    And('a profile "Mine" exists', () => {
      options = {
        ...options,
        profiles: [
          { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
          { id: 'mine', name: 'Mine', mod_ids: ['fast'] },
        ],
      };
    });
    And('a game session is running', async () => {
      const { session, controller } = await getWorld();
      session.emitSession({ state: 'started', cleaned: 0, started_at: null });
      await waitFor(() => expect(controller.getState().session.kind).toBe('running'));
    });
  });

  Scenario('Library Remove buttons are locked', ({ When, Then, And }) => {
    When('the user opens the Library', async () => {
      await getWorld();
      expect(screen.getByRole('heading', { name: 'Library' })).toBeInTheDocument();
    });
    Then('every Remove button is marked as disabled', () => {
      const buttons = screen.getAllByRole('button', { name: /^Remove / });
      expect(buttons).toHaveLength(2);
      for (const button of buttons) expect(button).toHaveAttribute('aria-disabled', 'true');
    });
    And('each Remove button explains that mods cannot be removed while the game is running', () => {
      for (const button of screen.getAllByRole('button', { name: /^Remove / })) {
        expect(button).toHaveAttribute('title', REMOVE_LOCKED);
        expect(button).toHaveAccessibleDescription(REMOVE_LOCKED);
      }
    });
    When('the user clicks Remove for "Rain Tyres"', async () => {
      const { user } = await getWorld();
      await user.click(removeButton('Rain Tyres'));
    });
    Then('no mod is removed', async () => {
      const { gateway } = await getWorld();
      expect(gateway.callsOf('removeMod')).toEqual([]);
      expect(gateway.mods).toHaveLength(2);
      expect(screen.getByText('Rain Tyres')).toBeInTheDocument();
    });
  });

  Scenario('Profile management is locked', ({ When, Then, And }) => {
    When('the user opens Profiles and selects "Mine"', async () => {
      const { user } = await getWorld();
      const nav = screen.getByRole('navigation', { name: 'Main navigation' });
      await user.click(within(nav).getByRole('button', { name: 'Profiles' }));
      await user.click(screen.getByRole('button', { name: 'Mine' }));
    });
    Then('the Create, Rename, Delete and Save actions are marked as disabled', () => {
      for (const button of profileActions()) expect(button).toHaveAttribute('aria-disabled', 'true');
    });
    And('each explains that profile changes are locked while the game is running', () => {
      for (const button of profileActions()) {
        expect(button).toHaveAttribute('title', PROFILES_LOCKED);
        expect(button).toHaveAccessibleDescription(PROFILES_LOCKED);
      }
    });
    When('the user tries to create, rename, delete and save a profile', async () => {
      const { user } = await getWorld();
      await user.type(screen.getByLabelText('Profile name'), 'Wet{Enter}');
      await user.type(screen.getByLabelText('New name'), 'X{Enter}');
      for (const button of profileActions()) await user.click(button);
    });
    Then('no profile change is sent', async () => {
      const { gateway } = await getWorld();
      for (const method of WRITES) expect(gateway.callsOf(method)).toEqual([]);
      expect(gateway.profiles.map((profile) => profile.name)).toEqual(['Vanilla', 'Mine']);
    });
  });

  Scenario('Actions unlock when the session ends', ({ Given, When, Then }) => {
    Given('the game session has ended', async () => {
      const { session, controller } = await getWorld();
      session.emitSession({ state: 'ended', cleaned: 1, started_at: null });
      await waitFor(() => expect(controller.getState().session.kind).toBe('ended'));
    });
    When('the user clicks Remove for "Rain Tyres"', async () => {
      const { user } = await getWorld();
      expect(removeButton('Rain Tyres')).not.toHaveAttribute('aria-disabled');
      await user.click(removeButton('Rain Tyres'));
    });
    Then('"Rain Tyres" is removed', async () => {
      const { gateway } = await getWorld();
      await waitFor(() => expect(gateway.callsOf('removeMod')).toHaveLength(1));
      await waitFor(() => expect(screen.queryByText('Rain Tyres')).not.toBeInTheDocument());
      expect(screen.getByText('Fast Bike')).toBeInTheDocument();
    });
  });

  Scenario('The new error code is translated', ({ Given, Then }) => {
    let locale = '';
    Given('the language is "pt-BR"', () => {
      locale = 'pt-BR';
    });
    Then('the error "library_locked_by_session" has a Portuguese message', () => {
      expect(locale).toBe('pt-BR');
      const message = ptBR.errors.library_locked_by_session;
      expect(message).toBe('Os mods não podem ser removidos da biblioteca enquanto o jogo está em execução.');
      expect(message).not.toBe(ptBR.errors.unknown);
    });
  });
});
