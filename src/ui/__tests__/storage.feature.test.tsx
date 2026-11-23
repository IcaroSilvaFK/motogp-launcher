// Binds tests/features/frontend/storage.feature: the real UI over a fake controller whose
// storage measurement is driven by the test (resolved, pending or rejected on demand).
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import type { CommandError, Locale, StorageUsage } from '../../domain';
import { App } from '../App';
import { createFakeController } from './fakeController';
import type { FakeController } from './fakeController';
import { loadFrontendFeature } from './features';

const KB = 1024;
const MB = 1024 * KB;
const GB = 1024 * MB;

// An accented Windows profile folder on purpose: real users have them and the path must render as is.
const ROOT = 'C:\\Users\\Usuário Padrão\\AppData\\Roaming\\motogp-launcher';

function usage(mods: number, launcherData: number, other: number, freeBytes: number | null): StorageUsage {
  return {
    root: ROOT,
    totalBytes: mods + launcherData + other,
    freeBytes,
    categories: [
      { category: 'mods', bytes: mods },
      { category: 'launcher_data', bytes: launcherData },
      { category: 'other', bytes: other },
    ],
  };
}

const reported = usage(1.5 * GB, 20 * KB, 0, 120 * GB);

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

let controller: FakeController;
let user: ReturnType<typeof userEvent.setup>;
let locale: Locale = 'en';

function prepare(measure: () => Promise<StorageUsage>) {
  controller = createFakeController();
  controller.getStorageUsage = vi.fn(measure);
  user = userEvent.setup();
}

async function openSettings(name = 'Settings') {
  render(<App controller={controller} locale={locale} />);
  const nav = within(screen.getByRole('navigation', { name: locale === 'en' ? 'Main navigation' : /navegação/i }));
  await user.click(nav.getByRole('button', { name }));
}

const section = (name = locale === 'en' ? 'Storage' : 'Armazenamento') => screen.getByRole('region', { name });
const card = () => section().querySelector('.storage-card') as HTMLElement;
/** The value (`<dd>`) of a stat whose term (`<dt>`) has this label. */
const stat = (label: string) =>
  within(section()).getByText(label, { selector: 'dt' }).nextElementSibling as HTMLElement;
const meterRow = (name: string) => within(section()).getByRole('meter', { name }).closest('li') as HTMLElement;

const feature = await loadFrontendFeature('storage');

describeFeature(feature, ({ Scenario, BeforeEachScenario, AfterEachScenario }) => {
  BeforeEachScenario(() => {
    locale = 'en';
  });
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  Scenario('The section shows the data folder, totals and the breakdown', ({ Given, When, Then, And }) => {
    Given(
      'the backend reports the data folder "C:\\Users\\Usuário Padrão\\AppData\\Roaming\\motogp-launcher", mods 1.5 GB, launcher data 20 KB, other 0 B and 120 GB free',
      () => prepare(() => Promise.resolve(reported)),
    );
    When('the user opens Settings', async () => {
      await openSettings();
      await waitFor(() => expect(stat('Total used')).toHaveTextContent('1.5 GB'));
    });
    Then('the Settings section nav lists "Storage"', () => {
      const sections = within(screen.getByRole('navigation', { name: 'Settings sections' }));
      expect(sections.getByRole('link', { name: 'Storage' })).toHaveAttribute('href', '#settings-storage');
    });
    And('the Storage section shows the data folder path', () => {
      const path = within(section()).getByText(ROOT);
      expect(path).toHaveAttribute('title', ROOT);
      expect(path).toHaveClass('storage-path');
    });
    And('it shows the total used and the free space', () => {
      expect(stat('Total used')).toHaveTextContent('1.5 GB');
      expect(stat('Free space on the drive')).toHaveTextContent('120 GB');
      expect(stat('Total used')).toHaveClass('num');
    });
    And('it shows one bar per category with its size and share', () => {
      const meters = within(section()).getAllByRole('meter');
      expect(meters.map((meter) => meter.getAttribute('aria-label'))).toEqual(['Mods', 'Launcher data', 'Other']);
      for (const meter of meters) {
        expect(meter).toHaveAttribute('aria-valuemin', '0');
        expect(meter).toHaveAttribute('aria-valuemax', '100');
      }
      expect(meters[0]).toHaveAttribute('aria-valuenow', '100');
      expect(meters[2]).toHaveAttribute('aria-valuenow', '0');
      expect(meterRow('Mods')).toHaveTextContent('1.5 GB');
      expect(meterRow('Mods')).toHaveTextContent('100%');
      expect(meterRow('Launcher data')).toHaveTextContent('20 KB');
      expect(meterRow('Launcher data')).toHaveTextContent('<1%');
      expect(meterRow('Other')).toHaveTextContent('0 B');
      expect(meterRow('Other')).toHaveTextContent('0%');
    });
  });

  Scenario('Open folder asks the backend to open the data folder', ({ Given, When, Then }) => {
    Given('the Storage section is shown', async () => {
      prepare(() => Promise.resolve(reported));
      await openSettings();
      await waitFor(() => expect(within(section()).getByText(ROOT)).toBeInTheDocument());
    });
    When('the user clicks "Open folder"', async () => {
      await user.click(within(section()).getByRole('button', { name: 'Open folder' }));
    });
    Then('the launcher asks to open the data folder', () => {
      expect(controller.openDataFolder).toHaveBeenCalledTimes(1);
      expect(controller.openDataFolder).toHaveBeenCalledWith();
    });
  });

  Scenario('Refresh measures again without blocking the screen', ({ Given, When, Then, And }) => {
    const second = deferred<StorageUsage>();

    Given('the Storage section is shown', async () => {
      let calls = 0;
      prepare(() => {
        calls += 1;
        return calls === 1 ? Promise.resolve(reported) : second.promise;
      });
      await openSettings();
      await waitFor(() => expect(stat('Total used')).toHaveTextContent('1.5 GB'));
    });
    When('the user clicks "Refresh" and the measurement is still running', async () => {
      await user.click(within(section()).getByRole('button', { name: 'Refresh' }));
      expect(controller.getStorageUsage).toHaveBeenCalledTimes(2);
    });
    Then('the Refresh button is disabled and the card is marked busy', () => {
      expect(within(section()).getByRole('button', { name: 'Refresh' })).toBeDisabled();
      expect(card()).toHaveAttribute('aria-busy', 'true');
    });
    And('the previous values stay visible', () => {
      expect(stat('Total used')).toHaveTextContent('1.5 GB');
      expect(within(section()).getAllByRole('meter')).toHaveLength(3);
      expect(section().querySelector('.skeleton')).toBeNull();
    });
    And('the other Settings controls stay enabled', () => {
      expect(screen.getByRole('button', { name: 'Detect games' })).toBeEnabled();
      expect(screen.getByRole('button', { name: 'Choose folder…' })).toBeEnabled();
      expect(screen.getByLabelText('Language')).toBeEnabled();
      expect(within(section()).getByRole('button', { name: 'Open folder' })).toBeEnabled();
    });
    When('the measurement finishes', () => {
      second.resolve(usage(2 * GB, 20 * KB, 0, 100 * GB));
    });
    Then('the new values are shown', async () => {
      await waitFor(() => expect(stat('Total used')).toHaveTextContent('2 GB'));
      expect(stat('Free space on the drive')).toHaveTextContent('100 GB');
      expect(within(section()).getByRole('button', { name: 'Refresh' })).toBeEnabled();
      expect(card()).toHaveAttribute('aria-busy', 'false');
    });
  });

  Scenario('Unknown free space is shown as unavailable', ({ Given, When, Then }) => {
    Given('the backend cannot read the free space', () => prepare(() => Promise.resolve(usage(10 * MB, KB, 0, null))));
    When('the user opens Settings', async () => {
      await openSettings();
      await waitFor(() => expect(stat('Total used')).toHaveTextContent('10 MB'));
    });
    Then('the free space shows "Unavailable"', () => {
      expect(stat('Free space on the drive')).toHaveTextContent('Unavailable');
    });
  });

  Scenario('A failed measurement shows an inline error with retry', ({ Given, When, Then, And }) => {
    const failure: CommandError = { code: 'read_failed', message: 'access denied' };

    Given('the backend fails to measure the storage', () => prepare(() => Promise.reject(failure)));
    When('the user opens Settings', async () => {
      await openSettings();
    });
    Then('the Storage section shows an error with a "Retry" button', async () => {
      const alert = await within(section()).findByRole('alert');
      expect(alert).toHaveTextContent('Could not measure the data folder.');
      expect(within(alert).getByRole('button', { name: 'Retry' })).toBeEnabled();
    });
    And('no global notice is shown', () => {
      expect(controller.getState().error).toBeNull();
      expect(document.querySelector('.app-messages .notice')).toBeNull();
      for (const alert of screen.getAllByRole('alert')) expect(section()).toContainElement(alert);
    });
  });

  Scenario('The section is translated', ({ Given, When, Then }) => {
    Given('the language is "pt-BR"', () => {
      locale = 'pt-BR';
      prepare(() => Promise.resolve(reported));
    });
    When('the user opens Settings', async () => {
      await openSettings('Configurações');
      await waitFor(() => expect(stat('Total usado')).toHaveTextContent('1,5 GB'));
    });
    Then('the Storage section labels are in Portuguese', () => {
      const region = section('Armazenamento');
      const sections = within(screen.getByRole('navigation', { name: 'Seções das configurações' }));
      expect(sections.getByRole('link', { name: 'Armazenamento' })).toBeInTheDocument();
      expect(within(region).getByText('Pasta de dados')).toBeInTheDocument();
      expect(within(region).getByRole('button', { name: 'Abrir pasta' })).toBeInTheDocument();
      expect(within(region).getByRole('button', { name: 'Atualizar' })).toBeInTheDocument();
      expect(within(region).getByText('Espaço livre no disco', { selector: 'dt' })).toBeInTheDocument();
      expect(
        within(region)
          .getAllByRole('meter')
          .map((meter) => meter.getAttribute('aria-label')),
      ).toEqual(['Mods', 'Dados do launcher', 'Outros']);
    });
  });
});
