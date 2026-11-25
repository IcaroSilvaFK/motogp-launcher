// Binds tests/features/frontend/notice-bar.feature end to end: the real launcher controller and the
// real UI, with only the ports replaced by in-memory fakes.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import type { MockInstance } from 'vitest';
import { createLauncherController } from '../../application';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { CommandError, Locale } from '../../domain';
import { App } from '../App';
import { loadFrontendFeature } from './features';

const RAW = 'os error 5: Access is denied';

let warnSpy: MockInstance<typeof console.warn> | null = null;

/** Opens the launcher, then makes the next profile save fail so the notice bar shows `error`. */
async function showError(locale: Locale, error: CommandError) {
  warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
  const gateway = new FakeGateway({
    mods: [{ id: 'rain', name: 'Rain Tyres', files: ['rain.pak'], size_bytes: 1024 }],
    profiles: [{ id: 'mine', name: 'Mine', mod_ids: [] }],
    config: { game_dir: null, active_profile: 'mine' },
  });
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
  });
  const user = userEvent.setup();
  render(<App controller={controller} locale={locale} />);
  await waitFor(() => expect(controller.getState().status).toBe('ready'));
  await waitFor(() => expect(controller.getState().busy).toBe(false));
  gateway.failNext('setProfileMods', error);
  await user.click(screen.getByRole('switch', { name: /Rain Tyres/ }));
  return await screen.findByRole('alert');
}

const feature = await loadFrontendFeature('notice-bar');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(() => {
    cleanup();
    warnSpy?.mockRestore();
    warnSpy = null;
  });

  Scenario('A known error shows only its translated message', ({ Given, And, When, Then }) => {
    let locale: Locale;
    let error: CommandError;
    let alert: HTMLElement;

    Given('the launcher is in English', () => {
      locale = 'en';
    });
    And('a command fails with code "write_failed" and message "os error 5: Access is denied"', () => {
      error = { code: 'write_failed', message: RAW };
    });
    When('the notice bar shows the error', async () => {
      alert = await showError(locale, error);
    });
    Then('it shows "Could not save the launcher data."', () => {
      expect(alert).toHaveTextContent('Could not save the launcher data.');
    });
    And('the raw error message is not shown', () => {
      expect(screen.queryByText(RAW, { exact: false })).not.toBeInTheDocument();
    });
  });

  Scenario('The translated message follows the selected language', ({ Given, And, When, Then }) => {
    let locale: Locale;
    let error: CommandError;
    let alert: HTMLElement;

    Given('the launcher is in Portuguese', () => {
      locale = 'pt-BR';
    });
    And('a command fails with code "write_failed" and message "os error 5: Access is denied"', () => {
      error = { code: 'write_failed', message: RAW };
    });
    When('the notice bar shows the error', async () => {
      alert = await showError(locale, error);
    });
    Then('it shows the Portuguese text for "write_failed"', () => {
      // pt-BR fixture: the Portuguese UI text is the expected output for this locale.
      expect(alert).toHaveTextContent('Não foi possível salvar os dados do launcher.');
    });
    And('the raw error message is not shown', () => {
      expect(screen.queryByText(RAW, { exact: false })).not.toBeInTheDocument();
    });
  });

  Scenario('The technical detail goes to the console', ({ Given, When, Then }) => {
    let error: CommandError;

    Given('a command fails with code "write_failed" and message "os error 5: Access is denied"', () => {
      error = { code: 'write_failed', message: RAW };
    });
    When('the notice bar shows the error', async () => {
      await showError('en', error);
    });
    Then('a console warning contains "write_failed" and "os error 5: Access is denied"', () => {
      expect(warnSpy).toHaveBeenCalledTimes(1);
      const message = String(warnSpy?.mock.calls[0]?.[0]);
      expect(message).toContain('write_failed');
      expect(message).toContain(RAW);
    });
  });

  Scenario('An unknown error code shows the generic fallback', ({ Given, When, Then, And }) => {
    let error: CommandError;
    let alert: HTMLElement;

    Given('a command fails with code "some_future_code" and message "boom"', () => {
      // A code the frontend does not know yet, as a newer backend could send.
      error = { code: 'some_future_code' as unknown as CommandError['code'], message: 'boom' };
    });
    When('the notice bar shows the error', async () => {
      alert = await showError('en', error);
    });
    Then('it shows the translated generic error message', () => {
      expect(alert).toHaveTextContent('Something went wrong.');
    });
    And('the raw error message is not shown', () => {
      expect(screen.queryByText('boom', { exact: false })).not.toBeInTheDocument();
    });
  });
});
