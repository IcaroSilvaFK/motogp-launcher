// Binds tests/features/frontend/restore-game.feature. The deployed-count and restore scenarios run the
// real controller over the in-memory gateway; the blocked and translated ones drive a fake controller.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect } from 'vitest';
import { createLauncherController } from '../../application';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { GameInfo, Locale } from '../../domain';
import { App } from '../App';
import { createFakeController } from './fakeController';
import type { FakeController } from './fakeController';
import { loadFrontendFeature } from './features';

const root = 'D:/Steam/MotoGP25';
const game: GameInfo = {
  name: 'MotoGP 25',
  root,
  mods_dir: `${root}/MotoGP25/Content/Paks/~mods`,
  app_id: 1234,
};

let user: ReturnType<typeof userEvent.setup>;
let gateway: FakeGateway;
let fake: FakeController;
let locale: Locale = 'en';

async function launchReal(tracked: number) {
  gateway = new FakeGateway({
    game,
    config: { game_dir: root, active_profile: 'vanilla' },
    manifest: { mods_dir: game.mods_dir, tracked },
    recovery: { state: 'recovered', error: null, cleaned_count: 0 },
  });
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
  });
  user = userEvent.setup();
  render(<App controller={controller} locale={locale} />);
  await waitFor(() => expect(controller.getState().status).toBe('ready'));
  await waitFor(() => expect(controller.getState().busy).toBe(false));
  await openSettings();
}

async function launchFake(patch: Parameters<FakeController['setState']>[0]) {
  fake = createFakeController();
  fake.setState({ game, ...patch });
  user = userEvent.setup();
  render(<App controller={fake} locale={locale} />);
  await openSettings();
}

async function openSettings() {
  const nav = within(screen.getByRole('navigation', { name: locale === 'en' ? 'Main navigation' : /navegação/i }));
  await user.click(nav.getByRole('button', { name: locale === 'en' ? 'Settings' : /configurações/i }));
}

const storage = () => screen.getByRole('region', { name: locale === 'en' ? 'Storage' : 'Armazenamento' });
const restoreButton = (name = 'Restore original game files') => within(storage()).getByRole('button', { name });

const feature = await loadFrontendFeature('restore-game');

describeFeature(feature, ({ BeforeEachScenario, AfterEachScenario, Scenario }) => {
  BeforeEachScenario(() => {
    locale = 'en';
  });
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  Scenario(
    'Storage shows that game files are safe and how many mod files are deployed',
    ({ Given, When, Then, And }) => {
      Given('3 mod files are deployed', () => undefined);
      When('the user opens Settings > Storage', () => launchReal(3));
      Then('the Game files row says the original game files are never changed', () => {
        expect(within(storage()).getByText('Game files')).toBeInTheDocument();
        expect(within(storage()).getByText(/never changes your original game files/)).toBeInTheDocument();
      });
      And('it shows "3 mod files deployed"', () => {
        expect(within(storage()).getByText('3 mod files deployed')).toBeInTheDocument();
      });
    },
  );

  Scenario('Restoring the original game', ({ Given, When, Then, And }) => {
    Given('3 mod files are deployed', () => launchReal(3));
    When('the user chooses "Restore original game files"', async () => {
      await user.click(restoreButton());
    });
    Then('the backend is asked to restore the game files', async () => {
      await waitFor(() => expect(gateway.callsOf('restoreGameFiles')).toHaveLength(1));
      expect(gateway.manifestState()).toBe('none');
    });
    And('a notice says how many files were removed', async () => {
      expect(await screen.findByText('Original game files restored; 3 files removed')).toBeInTheDocument();
      await waitFor(() => expect(within(storage()).getByText('No mod files deployed')).toBeInTheDocument());
    });
  });

  Scenario('Restore is blocked during a session', ({ Given, Then }) => {
    Given('the game is running', () => launchFake({ session: { kind: 'running', since: null } }));
    Then('"Restore original game files" is disabled with the reason', async () => {
      const button = restoreButton();
      expect(button).toHaveAttribute('aria-disabled', 'true');
      expect(button).toHaveAccessibleDescription('Wait for the game session to end before restoring.');
      await user.click(button);
      expect(fake.restoreGameFiles).not.toHaveBeenCalled();
    });
  });

  Scenario('Restore is blocked while an old session is pending', ({ Given, Then }) => {
    Given('a stale session from a previous install is pending', () =>
      launchFake({ staleSession: { modsDir: 'E:/Old/~mods', tracked: 3 } }),
    );
    Then('"Restore original game files" is disabled with a reason pointing to the old-session notice', async () => {
      const button = restoreButton();
      expect(button).toHaveAttribute('aria-disabled', 'true');
      expect(button).toHaveAccessibleDescription('Resolve the old session notice first.');
      await user.click(button);
      expect(fake.restoreGameFiles).not.toHaveBeenCalled();
    });
  });

  Scenario('The section is translated', ({ Given, Then }) => {
    Given('the language is "pt-BR"', async () => {
      locale = 'pt-BR';
      await launchFake({ session: { kind: 'running', since: null }, deployedFiles: 2 });
    });
    Then('the Game files row and its reasons are in Portuguese', () => {
      const section = within(storage());
      expect(section.getByText('Arquivos do jogo')).toBeInTheDocument();
      expect(section.getByText('2 arquivos de mod aplicados')).toBeInTheDocument();
      const button = section.getByRole('button', { name: 'Restaurar arquivos originais do jogo' });
      expect(button).toHaveAccessibleDescription('Aguarde o fim da sessão do jogo antes de restaurar.');
    });
  });
});
