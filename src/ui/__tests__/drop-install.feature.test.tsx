// Binds tests/features/frontend/drop-install.feature end to end: the real application controller
// and the real UI, with only the ports replaced by the in-memory fakes.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect } from 'vitest';
import { createLauncherController } from '../../application';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import { App } from '../App';
import { loadFrontendFeature } from './features';

async function launch() {
  const gateway = new FakeGateway();
  const drop = new FakeDropEvents();
  const controller = createLauncherController({
    gateway,
    picker: new FakePicker(),
    session: new FakeSessionEvents(),
    drop,
  });
  const user = userEvent.setup();
  render(<App controller={controller} locale="en" />);
  await waitFor(() => expect(controller.getState().status).toBe('ready'));
  await waitFor(() => expect(controller.getState().busy).toBe(false));
  await waitFor(() => expect(drop.activeListeners()).toBeGreaterThan(0));
  return { gateway, drop, controller, user };
}

type World = Awaited<ReturnType<typeof launch>>;

const overlay = () => screen.queryByRole('status', { name: 'Drop to install' });

const feature = await loadFrontendFeature('drop-install');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  let world: World;

  const showsLibrary = async () => {
    world = await launch();
    expect(screen.getByRole('heading', { name: 'Library' })).toBeInTheDocument();
  };
  const dragOver = async () => {
    world.drop.emitDragState(true);
    await waitFor(() => expect(overlay()).toBeInTheDocument());
  };
  const hidden = async () => {
    await waitFor(() => expect(overlay()).not.toBeInTheDocument());
  };
  const rejectedNotice = async (name: string) => {
    const notice = await screen.findByText(new RegExp(`${name.replace('.', '\\.')}.*not a mod archive`));
    expect(notice.closest('.notice')).toHaveClass('notice-warning');
  };

  Scenario('Dragging a file shows the drop overlay', ({ Given, When, Then }) => {
    Given('the launcher shows the Library', showsLibrary);
    When('a file is dragged over the launcher window', () => {
      world.drop.emitDragState(true);
    });
    Then('a "Drop to install" overlay is shown', async () => {
      const shown = await waitFor(() => {
        const element = overlay();
        expect(element).toBeInTheDocument();
        return element as HTMLElement;
      });
      expect(shown).toHaveClass('drop-overlay');
      expect(shown).toHaveTextContent('.pak, .utoc, .ucas or .zip');
    });
  });

  Scenario('Leaving the window hides the overlay', ({ Given, And, When, Then }) => {
    Given('the launcher shows the Library', showsLibrary);
    And('a file is being dragged over the launcher window', dragOver);
    When('the drag leaves the window', () => {
      world.drop.emitDragState(false);
    });
    Then('the overlay is hidden', hidden);
  });

  Scenario('Dropping the file hides the overlay', ({ Given, And, When, Then }) => {
    Given('the launcher shows the Library', showsLibrary);
    And('a file is being dragged over the launcher window', dragOver);
    When('the file is dropped', () => {
      world.drop.emitDrop(['C:/x/a.pak']);
    });
    Then('the overlay is hidden', hidden);
  });

  Scenario('Esc hides the overlay', ({ Given, And, When, Then }) => {
    Given('the launcher shows the Library', showsLibrary);
    And('a file is being dragged over the launcher window', dragOver);
    When('the user presses Esc', async () => {
      await world.user.keyboard('{Escape}');
    });
    Then('the overlay is hidden', hidden);
  });

  Scenario('Non-zip files are rejected with a message', ({ Given, When, Then, And }) => {
    Given('the launcher shows the Library', showsLibrary);
    When('"readme.txt" and "livery.zip" are dropped', () => {
      world.drop.emitDrop(['C:\\Downloads\\readme.txt', 'C:\\Downloads\\livery.zip']);
    });
    Then('"livery.zip" is installed', async () => {
      await waitFor(() =>
        expect(world.gateway.callsOf('importMods')[0]?.args).toEqual([['C:\\Downloads\\livery.zip']]),
      );
    });
    And('a notice says "readme.txt" is not a mod archive', () => rejectedNotice('readme.txt'));
  });

  Scenario('Only rejected files dropped', ({ Given, When, Then, And }) => {
    Given('the launcher shows the Library', showsLibrary);
    When('only "readme.txt" is dropped', () => {
      world.drop.emitDrop(['C:/Downloads/readme.txt']);
    });
    Then('nothing is installed', () => {
      expect(world.gateway.callsOf('importMods')).toHaveLength(0);
    });
    And('a notice says "readme.txt" is not a mod archive', () => rejectedNotice('readme.txt'));
  });
});
