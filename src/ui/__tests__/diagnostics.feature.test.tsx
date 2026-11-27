// Binds tests/features/frontend/diagnostics.feature: the real UI over a fake controller and the
// in-memory Diagnostics port (clipboard and logs folder are recorded, never touched).
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import type { Locale } from '../../domain';
import { FakeDiagnostics } from '../../application/__tests__/fakeDiagnostics';
import { App } from '../App';
import { createFakeController } from './fakeController';
import type { FakeController } from './fakeController';
import { loadFrontendFeature } from './features';

// An accented Windows folder on purpose: real users have them and the report must keep them.
const GAME_ROOT = 'D:/Steam/Usuário Padrão/MotoGP25';
const SESSION_TOKEN = 'sess-9f86d081884c7d659a2feaa0c55ad015';

let controller: FakeController;
let diagnostics: FakeDiagnostics;
let user: ReturnType<typeof userEvent.setup>;
let locale: Locale = 'en';

function launch({ withGame = true } = {}) {
  controller = createFakeController(
    withGame
      ? {
          config: { game_dir: GAME_ROOT, active_profile: 'vanilla' },
          game: { name: 'MotoGP 25', root: GAME_ROOT, mods_dir: `${GAME_ROOT}/~mods`, app_id: 1234 },
          mods: [
            { id: 'a', name: 'A', files: [], size_bytes: 1 },
            { id: 'b', name: 'B', files: [], size_bytes: 1 },
          ],
        }
      : {},
  );
  diagnostics = new FakeDiagnostics();
  diagnostics.snapshotResult = { os: 'Windows 11', logLines: ['2026-11-27T10:00:00Z INFO started'] };
  user = userEvent.setup();
}

async function openSettings() {
  render(
    <App
      controller={controller}
      locale={locale}
      appInfo={{ getVersion: () => Promise.resolve('0.1.0') }}
      diagnostics={diagnostics}
    />,
  );
  const nav = within(screen.getByRole('navigation', { name: locale === 'en' ? 'Main navigation' : /navegação/i }));
  await user.click(nav.getByRole('button', { name: locale === 'en' ? 'Settings' : 'Configurações' }));
}

const region = (name: string) => within(screen.getByRole('region', { name }));
const aboutRegion = () => region(locale === 'en' ? 'About' : 'Sobre');
const storageRegion = () => region(locale === 'en' ? 'Storage' : 'Armazenamento');

async function clickCopy() {
  await user.click(aboutRegion().getByRole('button', { name: 'Copy diagnostics' }));
}

const feature = await loadFrontendFeature('diagnostics');

describeFeature(feature, ({ Scenario, BeforeEachScenario, AfterEachScenario }) => {
  BeforeEachScenario(() => {
    locale = 'en';
  });
  AfterEachScenario(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  Scenario('Copy diagnostics', ({ Given, When, Then, And }) => {
    Given('the launcher version is "0.1.0", the OS is "Windows 11" and a game is configured', () => launch());
    When('the user clicks "Copy diagnostics" in About', async () => {
      await openSettings();
      await waitFor(() => expect(aboutRegion().getByText('0.1.0')).toBeInTheDocument());
      await clickCopy();
    });
    Then(
      'a text with the app version, OS, game folder, mod count, active profile and recent log lines is copied',
      () => {
        expect(diagnostics.copied).toHaveLength(1);
        const text = diagnostics.copied[0];
        expect(text).toContain('Launcher version: 0.1.0');
        expect(text).toContain('OS: Windows 11');
        expect(text).toContain(`Game folder: ${GAME_ROOT}`);
        expect(text).toContain('Installed mods: 2');
        expect(text).toContain('Active profile: Vanilla');
        expect(text).toContain('2026-11-27T10:00:00Z INFO started');
      },
    );
    And('"Copied" is shown', () => {
      expect(aboutRegion().getByText('Copied')).toBeInTheDocument();
    });
  });

  Scenario('Diagnostics without a game folder', ({ Given, When, Then }) => {
    Given('no game folder is configured', () => launch({ withGame: false }));
    When('the user clicks "Copy diagnostics" in About', async () => {
      await openSettings();
      await clickCopy();
    });
    Then('the copied text says the game folder was not detected', () => {
      expect(diagnostics.copied[0]).toContain('Game folder: not detected');
      expect(diagnostics.copied[0]).toContain('Active profile: none');
    });
  });

  Scenario('Copying fails', ({ Given, When, Then }) => {
    Given('the clipboard is not available', () => {
      launch();
      diagnostics.failCopy = true;
      vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    });
    When('the user clicks "Copy diagnostics" in About', async () => {
      await openSettings();
      await clickCopy();
    });
    Then('an error is shown instead of "Copied"', async () => {
      await waitFor(() => expect(aboutRegion().getByText('Could not copy to the clipboard.')).toBeInTheDocument());
      expect(aboutRegion().queryByText('Copied')).not.toBeInTheDocument();
    });
  });

  Scenario('Open logs folder', ({ Given, When, Then }) => {
    Given('the Settings screen is open', () => launch());
    When('the user clicks "Open logs folder" in Settings > Storage', async () => {
      await openSettings();
      await user.click(storageRegion().getByRole('button', { name: 'Open logs folder' }));
    });
    Then('the folder holding the log files opens', () => {
      expect(diagnostics.logsFolderOpened).toBe(1);
    });
  });

  Scenario('No secrets in diagnostics', ({ Given, When, Then }) => {
    Given('the backend already removed the session token from the log lines', () => {
      launch();
      // The backend redacts; the report adds only the facts above and these lines.
      diagnostics.snapshotResult = { os: 'Windows 11', logLines: ['INFO sign-in ok token=[redacted]'] };
    });
    When('the user clicks "Copy diagnostics" in About', async () => {
      await openSettings();
      await clickCopy();
    });
    Then('the copied text never contains the community session token', () => {
      expect(diagnostics.copied[0]).not.toContain(SESSION_TOKEN);
      expect(diagnostics.copied[0]).toContain('token=[redacted]');
    });
  });

  Scenario('The labels follow the language', ({ Given, Then }) => {
    Given('the language is Portuguese', async () => {
      locale = 'pt-BR';
      launch();
      await openSettings();
    });
    Then('the diagnostics actions are labeled in Portuguese', () => {
      expect(aboutRegion().getByRole('button', { name: 'Copiar diagnóstico' })).toBeInTheDocument();
      expect(storageRegion().getByRole('button', { name: 'Abrir pasta de logs' })).toBeInTheDocument();
    });
  });
});
