// Binds tests/features/frontend/language.feature: the real controller and UI, wired to an
// in-memory LocaleStore through loadLocale/saveLocale exactly like src/main.tsx does.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect } from 'vitest';
import { createLauncherController, loadLocale, saveLocale } from '../../application';
import type { LocaleStore } from '../../application';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { Locale } from '../../domain';
import { App } from '../App';
import { loadFrontendFeature } from './features';

function launch(store: LocaleStore) {
  const controller = createLauncherController({
    gateway: new FakeGateway(),
    picker: new FakePicker(),
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
  });
  return {
    controller,
    ...render(
      <App controller={controller} locale={loadLocale(store)} onLocaleChange={(locale) => saveLocale(store, locale)} />,
    ),
  };
}

const feature = await loadFrontendFeature('language');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  // Steps are separate tests: the DOM must survive between them, so clean up per scenario.
  AfterEachScenario(cleanup);

  Scenario('The language survives a restart', ({ Given, When, Then }) => {
    let saved: Locale | null = null;
    const store: LocaleStore = { load: () => saved, save: (locale) => void (saved = locale) };

    Given('the user switched the language to English', async () => {
      const first = launch(store);
      await waitFor(() => expect(first.controller.getState().status).toBe('ready'));
      await userEvent.click(screen.getByRole('button', { name: 'Configurações' }));
      await userEvent.selectOptions(screen.getByLabelText('Idioma'), 'en');
      expect(screen.getByRole('button', { name: 'Play' })).toBeInTheDocument();
      first.unmount();
    });

    When('the launcher restarts', async () => {
      const second = launch(store);
      await waitFor(() => expect(second.controller.getState().status).toBe('ready'));
    });

    Then('the interface is shown in English', () => {
      expect(screen.getByRole('button', { name: 'Play' })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Settings' })).toBeInTheDocument();
    });
  });
});
