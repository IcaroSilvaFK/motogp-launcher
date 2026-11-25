// Binds tests/features/frontend/support.feature: the real App over a fake controller. The support
// page address is a backend constant, so every assertion checks the UI passes no argument at all.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import type { NavPreferenceStore } from '../../application';
import type { Locale } from '../../domain';
import { App } from '../App';
import { createFakeController } from './fakeController';
import type { FakeController } from './fakeController';
import { loadFrontendFeature } from './features';

const OPEN_FAILED = 'The page could not be opened in your browser.';

function navStore(collapsed: boolean): NavPreferenceStore {
  return { load: () => collapsed, save: () => undefined };
}

interface World {
  controller: FakeController;
  user: ReturnType<typeof userEvent.setup>;
}

function launch({ collapsed = false, locale = 'en' }: { collapsed?: boolean; locale?: Locale } = {}): World {
  const controller = createFakeController();
  const user = userEvent.setup();
  render(<App controller={controller} locale={locale} navStore={navStore(collapsed)} />);
  return { controller, user };
}

const sidebar = () => within(screen.getByRole('complementary'));
// pt-BR fixture: the Portuguese UI labels are accepted on purpose (real users run the pt-BR UI).
const mainNav = () => within(screen.getByRole('navigation', { name: /Main navigation|Navegação principal/ }));
const about = (name = 'About') => within(screen.getByRole('region', { name }));

async function openSettings(world: World, label = 'Settings') {
  await world.user.click(mainNav().getByRole('button', { name: label }));
}

const feature = await loadFrontendFeature('support');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  Scenario('Support entry in the expanded sidebar footer', ({ Given, Then }) => {
    Given('the sidebar is expanded', () => {
      launch();
      expect(screen.getByRole('complementary')).toHaveAttribute('data-collapsed', 'false');
    });

    Then('the footer shows a "Support the project" entry', () => {
      const entry = sidebar().getByRole('button', { name: 'Support the project' });
      // A footer action, not a navigation item: outside the main nav and never "current".
      expect(mainNav().queryByRole('button', { name: 'Support the project' })).toBeNull();
      expect(entry).not.toHaveAttribute('aria-current');
      expect(entry.closest('.sidebar-footer')).not.toBeNull();
      expect(within(entry).getByText('Support the project')).toBeVisible();
      expect(within(entry).getByText('Support the project')).not.toHaveClass('visually-hidden');
      expect(entry).toHaveAccessibleDescription('Opens in your browser');
    });
  });

  Scenario('Support entry in the collapsed sidebar', ({ Given, Then }) => {
    Given('the sidebar is collapsed', () => {
      launch({ collapsed: true });
      expect(screen.getByRole('complementary')).toHaveAttribute('data-collapsed', 'true');
    });

    Then('the footer shows only the support icon with an accessible name and tooltip', () => {
      const entry = sidebar().getByRole('button', { name: 'Support the project' });
      expect(entry).toHaveAttribute('title', 'Support the project');
      expect(entry.querySelector('svg')).not.toBeNull();
      expect(within(entry).getByText('Support the project')).toHaveClass('visually-hidden');
    });
  });

  Scenario('Support entry in Settings > About', ({ When, Then }) => {
    let world: World;

    When('the user opens Settings', async () => {
      world = launch();
      await openSettings(world);
    });

    Then('About shows a short support text and a "Support on Apoia.se" button', () => {
      const button = about().getByRole('button', { name: 'Support on Apoia.se' });
      const row = button.closest('.setting-row') as HTMLElement;
      expect(row).not.toBeNull();
      expect(within(row).getByText('Support the project')).toBeInTheDocument();
      expect(within(row).getByText(/is free/)).toBeInTheDocument();
      // Quiet on purpose: never a filled primary button next to the version and updates.
      expect(button).not.toHaveClass('btn-primary');
      expect(button).toHaveAccessibleDescription('Opens in your browser');
    });
  });

  Scenario('Clicking the entry opens the support page', ({ When, Then }) => {
    let world: World;

    When('the user activates any support entry', async () => {
      world = launch();
      await world.user.click(sidebar().getByRole('button', { name: 'Support the project' }));
      await openSettings(world);
      // Keyboard activation of the About button.
      about().getByRole('button', { name: 'Support on Apoia.se' }).focus();
      await world.user.keyboard('{Enter}');
    });

    Then('the controller is asked to open the support page, with no URL', () => {
      expect(world.controller.openSupportPage).toHaveBeenCalledTimes(2);
      for (const call of vi.mocked(world.controller.openSupportPage).mock.calls) {
        expect(call).toEqual([]);
      }
    });
  });

  Scenario('The browser cannot be opened', ({ Given, When, Then }) => {
    let world: World;

    Given('opening the support page fails', () => {
      world = launch();
      world.controller.openSupportPage = vi
        .fn()
        .mockRejectedValueOnce({ code: 'open_page_failed', message: 'shell refused' })
        .mockRejectedValueOnce({ code: 'open_page_failed', message: 'shell refused' })
        .mockResolvedValue(undefined);
    });

    When('the user activates the support entry', async () => {
      await world.user.click(sidebar().getByRole('button', { name: 'Support the project' }));
    });

    Then('a non-blocking message says the page could not be opened', async () => {
      // Sidebar: an inline alert in the footer, no dialog, the rest of the launcher still usable.
      expect(await sidebar().findByRole('alert')).toHaveTextContent(OPEN_FAILED);
      expect(screen.queryByRole('dialog')).toBeNull();
      expect(screen.queryByRole('alertdialog')).toBeNull();
      expect(screen.queryByText('shell refused', { exact: false })).toBeNull();

      // About: the same inline message next to its button, cleared by the next successful try.
      await openSettings(world);
      const button = about().getByRole('button', { name: 'Support on Apoia.se' });
      await world.user.click(button);
      expect(await about().findByRole('alert')).toHaveTextContent(OPEN_FAILED);
      await world.user.click(button);
      await waitFor(() => expect(about().queryByRole('alert')).toBeNull());
      expect(world.controller.openSupportPage).toHaveBeenCalledTimes(3);
    });
  });

  Scenario('Portuguese copy', ({ Given, Then }) => {
    let world: World;

    Given('the language is pt-BR', () => {
      world = launch({ locale: 'pt-BR' });
    });

    Then('the support entry and the About text are shown in Portuguese', async () => {
      // pt-BR fixture: the Portuguese labels below are the expected text for this locale.
      const entry = sidebar().getByRole('button', { name: 'Apoie o projeto' });
      expect(entry).toHaveAccessibleDescription('Abre no navegador');
      await openSettings(world, 'Configurações');
      const button = about('Sobre').getByRole('button', { name: 'Apoiar no Apoia.se' });
      const row = button.closest('.setting-row') as HTMLElement;
      expect(within(row).getByText('Apoie o projeto')).toBeInTheDocument();
      expect(within(row).getByText(/é gratuito/)).toBeInTheDocument();
    });
  });
});
