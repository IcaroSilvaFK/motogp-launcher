// Binds tests/features/frontend/mod-hosting.feature (#130): the real App, the real catalog controller
// over a scripted gateway, the real publish hooks, and in-memory fakes of the CommunityAccount,
// ModPublisher and PublishFilePicker ports. The server's half (size cap, resumable uploads, review)
// is covered by the server tests; here the UI shows what the ports answer.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import { createCatalogController } from '../../application';
import type { CatalogGateway } from '../../application';
import type { Catalog, CatalogEntry, CommunityStatus } from '../../domain';
import { App } from '../App';
import { FakeCommunityAccount, signedIn, signedOut } from './fakeCommunity';
import { createFakeController } from './fakeController';
import { FakeModPublisher, FakePublishPicker, FILE_SIZE, version } from './fakePublisher';
import { loadFrontendFeature } from './features';

const livery: CatalogEntry = {
  id: 'classic-livery',
  title: 'Classic Livery',
  author: 'Paint Shop',
  game: 'MotoGP 26',
  category: 'liveries',
  description: 'A classic livery pack.',
  version: '1.0.0',
  updatedAt: '2026-11-01',
  pageUrl: 'https://overtake.gg/downloads/classic-livery/',
  thumbnailUrl: null,
  screenshots: [],
  longDescription: null,
  download: null,
};

const catalog: Catalog = {
  entries: [livery],
  fetchedAt: Date.UTC(2026, 10, 24, 12),
  stale: false,
  source: 'remote',
  skipped: 0,
};

interface World {
  account: FakeCommunityAccount;
  publisher: FakeModPublisher;
  picker: FakePublishPicker;
  user: ReturnType<typeof userEvent.setup>;
}

function setUp(status: CommunityStatus): World {
  return {
    account: new FakeCommunityAccount(status),
    publisher: new FakeModPublisher(),
    picker: new FakePublishPicker(),
    user: userEvent.setup(),
  };
}

const nav = () => within(screen.getByRole('navigation', { name: 'Main navigation' }));

async function openDiscover(world: World) {
  const gateway: CatalogGateway = {
    getCatalog: vi.fn(() => Promise.resolve(catalog)),
    refreshCatalog: vi.fn(() => Promise.resolve(catalog)),
    openCatalogPage: vi.fn(() => Promise.resolve()),
  };
  render(
    <App
      controller={createFakeController()}
      catalog={createCatalogController({ gateway })}
      communityAccount={world.account}
      modPublisher={world.publisher}
      publishPicker={world.picker}
      locale="en"
    />,
  );
  await world.user.click(nav().getByRole('button', { name: 'Discover' }));
  await screen.findByRole('list', { name: /results/i });
}

async function openPublish(world: World) {
  await openDiscover(world);
  await world.user.click(await screen.findByRole('button', { name: 'Publish a mod' }));
  return screen.getByRole('region', { name: 'Publish a mod' });
}

/**
 * Fills every field of a valid form for "Brazil Livery" with a zip file. Text goes in with one paste
 * per field instead of one keystroke per character: typing ~60 characters re-renders the whole App
 * on each key, which made the steps that call this the slowest in the file and pushed them past the
 * step timeout on a loaded machine (#162). The timeout is unchanged; the steps just do less work.
 */
async function fillForm(world: World, versionText = '1.0') {
  const { user } = world;
  const fill = async (label: string, text: string) => {
    await user.click(screen.getByLabelText(label));
    await user.paste(text);
  };
  await fill('Title', 'Brazil Livery');
  await fill('Short description', 'Green and yellow.');
  await fill('Category', 'liveries');
  await user.clear(screen.getByLabelText('Game'));
  await fill('Game', 'MotoGP 26');
  await fill('Version', versionText);
  await user.click(screen.getByRole('button', { name: 'Choose mod file' }));
  await within(screen.getByRole('tabpanel', { name: 'Publish' })).findByText('brazil-livery.zip');
  await user.click(screen.getByRole('checkbox', { name: /I made this mod/ }));
}

const publishButton = () => screen.getByRole('button', { name: 'Publish' });
const tab = (name: string) => screen.getByRole('tab', { name: new RegExp(`^${name}`) });

const feature = await loadFrontendFeature('mod-hosting');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(cleanup);

  Scenario('Only a signed-in creator sees the publish entry', ({ Given, When, Then }) => {
    let world: World;
    Given('the creator is signed out', () => {
      world = setUp(signedOut());
    });
    When('they open Discover', async () => {
      await openDiscover(world);
    });
    Then('there is no "Publish a mod" button', async () => {
      await waitFor(() => expect(world.account.status).toHaveBeenCalled());
      expect(screen.queryByRole('button', { name: 'Publish a mod' })).not.toBeInTheDocument();
    });
    When('they sign in from Settings and open Discover again', async () => {
      await world.user.click(nav().getByRole('button', { name: 'Settings' }));
      await world.user.click(await screen.findByRole('button', { name: 'Sign in with Discord' }));
      await screen.findByRole('button', { name: 'Sign out' });
      await world.user.click(nav().getByRole('button', { name: 'Discover' }));
    });
    Then('Discover shows a "Publish a mod" button', async () => {
      expect(await screen.findByRole('button', { name: 'Publish a mod' })).toBeInTheDocument();
    });
  });

  Scenario('A creator publishes a mod', ({ Given, When, Then, And }) => {
    let world: World;
    Given('the creator is signed in', async () => {
      world = setUp(signedIn());
      await openPublish(world);
    });
    When('they upload "Brazil Livery" version 1.0 with a zip file', async () => {
      await fillForm(world);
      expect(screen.getByLabelText('Mod id')).toHaveValue('brazil-livery');
      await world.user.click(publishButton());
    });
    Then('they see that the mod is waiting for review', async () => {
      expect(await screen.findByRole('heading', { name: 'Sent for review' })).toBeInTheDocument();
      expect(screen.getByText(/Brazil Livery 1\.0 is waiting for review/)).toBeInTheDocument();
      expect(world.publisher.publish).toHaveBeenCalledWith(
        expect.objectContaining({
          modId: 'brazil-livery',
          title: 'Brazil Livery',
          version: '1.0',
          file: 'C:\\Mods\\brazil-livery.zip',
          ownershipConfirmed: true,
        }),
      );
    });
    And('it appears in "My mods" as waiting for review', async () => {
      await world.user.click(screen.getByRole('button', { name: 'View my mods' }));
      expect(tab('My mods')).toHaveAttribute('aria-selected', 'true');
      const mod = await screen.findByRole('article', { name: 'Brazil Livery' });
      const row = within(mod).getByRole('listitem', { name: 'Version 1.0' });
      expect(within(row).getByText('Waiting for review')).toBeInTheDocument();
    });
  });

  Scenario('The mod id follows the title until the creator edits it', ({ Given, When, Then }) => {
    let world: World;
    Given('the creator is signed in and opens the publish form', async () => {
      world = setUp(signedIn());
      await openPublish(world);
    });
    When('they type the title "Pista Épica 2025"', async () => {
      await world.user.type(screen.getByLabelText('Title'), 'Pista Épica 2025');
    });
    Then('the mod id reads "pista-epica-2025"', () => {
      expect(screen.getByLabelText('Mod id')).toHaveValue('pista-epica-2025');
    });
    When('they change the mod id to "my-track" and edit the title', async () => {
      await world.user.clear(screen.getByLabelText('Mod id'));
      await world.user.type(screen.getByLabelText('Mod id'), 'my-track');
      await world.user.type(screen.getByLabelText('Title'), ' Pro');
    });
    Then('the mod id still reads "my-track"', () => {
      expect(screen.getByLabelText('Mod id')).toHaveValue('my-track');
    });
  });

  Scenario('An incomplete form shows what to fix', ({ Given, When, Then, And }) => {
    let world: World;
    Given('the creator is signed in and opens the publish form', async () => {
      world = setUp(signedIn());
      await openPublish(world);
    });
    When('they choose Publish with an empty form', async () => {
      await world.user.clear(screen.getByLabelText('Game'));
      await world.user.click(publishButton());
    });
    Then('each required field shows its error next to it', () => {
      const title = screen.getByLabelText('Title');
      expect(title).toHaveAttribute('aria-invalid', 'true');
      expect(title).toHaveAccessibleDescription(expect.stringContaining('Enter a title.'));
      expect(title).toHaveFocus();
      expect(screen.getByLabelText('Version')).toHaveAccessibleDescription(expect.stringContaining('Enter a version.'));
      expect(screen.getByLabelText('Game')).toHaveAccessibleDescription(expect.stringContaining('Choose a game.'));
      expect(screen.getByText('Choose the mod file.')).toBeInTheDocument();
      expect(screen.getByRole('checkbox', { name: /I made this mod/ })).toHaveAccessibleDescription(
        'Confirm that you own this mod or have the right to share it.',
      );
    });
    And('nothing is uploaded', () => {
      expect(world.publisher.publish).not.toHaveBeenCalled();
    });
  });

  Scenario('Screenshots can be added and removed', ({ Given, When, Then }) => {
    let world: World;
    Given('the creator is signed in and opens the publish form', async () => {
      world = setUp(signedIn());
      await openPublish(world);
    });
    When('they add two screenshots and remove the first one', async () => {
      world.picker.nextScreenshots = ['C:\\Shots\\front.png', 'C:\\Shots\\side.jpg'];
      await world.user.click(screen.getByRole('button', { name: 'Add screenshots' }));
      await screen.findByText('front.png');
      expect(screen.getByText('2 of 8')).toBeInTheDocument();
      await world.user.click(screen.getByRole('button', { name: 'Remove front.png' }));
    });
    Then('only the second screenshot is listed', () => {
      const list = screen.getByRole('list', { name: 'Screenshots' });
      expect(within(list).getAllByRole('listitem')).toHaveLength(1);
      expect(within(list).getByText('side.jpg')).toBeInTheDocument();
    });
  });

  Scenario('The upload shows its progress and can be cancelled', ({ Given, When, Then, And }) => {
    let world: World;
    Given('the creator is signed in and a publish is uploading', async () => {
      world = setUp(signedIn());
      world.publisher.hold();
      await openPublish(world);
      await fillForm(world);
      await world.user.click(publishButton());
      act(() => world.publisher.emit({ sent: FILE_SIZE / 2, total: FILE_SIZE, phase: 'uploading' }));
    });
    Then('a progress bar shows the bytes sent and the total', () => {
      const bar = screen.getByRole('progressbar', { name: 'Upload progress' });
      expect(bar).toHaveAttribute('aria-valuenow', '50');
      expect(bar).toHaveAttribute('aria-valuetext', 'Uploading 1 MB of 2 MB');
    });
    And('the form cannot be submitted again', () => {
      expect(screen.getByRole('button', { name: /Publishing/ })).toBeDisabled();
      expect(screen.getByLabelText('Title')).toBeDisabled();
      expect(screen.getByRole('button', { name: 'Back to Discover' })).toBeDisabled();
    });
    When('they cancel the upload', async () => {
      await world.user.click(screen.getByRole('button', { name: 'Cancel upload' }));
    });
    Then('they see that the upload was cancelled and the form is kept', async () => {
      expect(await screen.findByText(/Upload cancelled/)).toBeInTheDocument();
      expect(screen.getByLabelText('Title')).toHaveValue('Brazil Livery');
      expect(publishButton()).toBeEnabled();
    });
  });

  Scenario('A failed upload explains why and can be retried', ({ Given, When, Then }) => {
    let world: World;
    Given('the creator is signed in and the server already has this version', async () => {
      world = setUp(signedIn());
      world.publisher.stored.set('brazil-livery', [version({ modId: 'brazil-livery', version: '1.0' })]);
      await openPublish(world);
    });
    When('they publish the mod', async () => {
      await fillForm(world);
      await world.user.click(publishButton());
    });
    Then('they see that the version was already published', async () => {
      const alert = await screen.findByRole('alert');
      expect(alert).toHaveTextContent('This version of the mod was already published');
    });
    When('they change the version and try again', async () => {
      await world.user.clear(screen.getByLabelText('Version'));
      await world.user.type(screen.getByLabelText('Version'), '1.1');
      await world.user.click(within(screen.getByRole('alert')).getByRole('button', { name: 'Try again' }));
    });
    Then('the mod is waiting for review', async () => {
      expect(await screen.findByRole('heading', { name: 'Sent for review' })).toBeInTheDocument();
      expect(world.publisher.stored.get('brazil-livery')?.map((v) => v.version)).toEqual(['1.0', '1.1']);
    });
  });

  Scenario('My mods shows every version with its review status', ({ Given, When, Then, And }) => {
    let world: World;
    Given('the creator has a mod with an approved, a rejected and a pending version', () => {
      world = setUp(signedIn());
      world.publisher.stored.set('brazil-livery', [
        version({ modId: 'brazil-livery', version: '1.0', status: 'approved', reviewedAt: '2026-11-21T10:00:00Z' }),
        version({ modId: 'brazil-livery', version: '1.1', status: 'rejected', rejectionReason: 'Missing credits.' }),
        version({ modId: 'brazil-livery', version: '1.2' }),
      ]);
    });
    When('they open "My mods"', async () => {
      await openPublish(world);
      await world.user.click(tab('My mods'));
    });
    Then('each version shows its status', async () => {
      const mod = await screen.findByRole('article', { name: 'Brazil Livery' });
      const row = (v: string) => within(mod).getByRole('listitem', { name: `Version ${v}` });
      expect(within(row('1.0')).getByText('Approved')).toBeInTheDocument();
      expect(within(row('1.1')).getByText('Rejected')).toBeInTheDocument();
      expect(within(row('1.2')).getByText('Waiting for review')).toBeInTheDocument();
      expect(tab('My mods')).toHaveTextContent('1');
    });
    And('the rejected version shows the reason', () => {
      const mod = screen.getByRole('article', { name: 'Brazil Livery' });
      const row = within(mod).getByRole('listitem', { name: 'Version 1.1' });
      expect(within(row).getByText('Reason: Missing credits.')).toBeInTheDocument();
    });
  });

  Scenario('My mods explains an empty list and a failed load', ({ Given, When, Then }) => {
    let world: World;
    Given('the creator has never published a mod', () => {
      world = setUp(signedIn());
    });
    When('they open "My mods"', async () => {
      await openPublish(world);
      await world.user.click(tab('My mods'));
    });
    Then('they are told they have no mods yet', async () => {
      expect(await screen.findByText('You have not published any mods yet')).toBeInTheDocument();
    });
    When('the server cannot be reached and they refresh', async () => {
      world.publisher.offline = true;
      await world.user.click(screen.getByRole('button', { name: 'Refresh' }));
    });
    Then('they see the error and can try again', async () => {
      expect(await screen.findByText(/community server cannot be reached/)).toBeInTheDocument();
      world.publisher.offline = false;
      await world.user.click(screen.getByRole('button', { name: 'Try again' }));
      expect(await screen.findByText('You have not published any mods yet')).toBeInTheDocument();
    });
  });

  Scenario('Escape goes back to Discover', ({ Given, When, Then }) => {
    let world: World;
    Given('the creator is signed in and opens the publish form', async () => {
      world = setUp(signedIn());
      await openPublish(world);
      expect(screen.getByRole('button', { name: 'Back to Discover' })).toHaveFocus();
    });
    When('they press Escape', async () => {
      await world.user.keyboard('{Escape}');
    });
    Then('Discover lists the mods again', async () => {
      expect(await screen.findByRole('list', { name: /results/i })).toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Publish a mod' })).toHaveFocus();
    });
  });

  Scenario('A suspended creator sees why publishing is refused', ({ Given, When, Then }) => {
    let world: World;
    Given('the creator is signed in but their community account is suspended', async () => {
      world = setUp(signedIn());
      world.publisher.banned = true;
      await openPublish(world);
    });
    When('they publish the mod', async () => {
      await fillForm(world);
      await world.user.click(publishButton());
    });
    Then('they see that the account is suspended, with no retry', async () => {
      const alert = await screen.findByRole('alert');
      expect(alert).toHaveTextContent("Your community account is suspended, so you can't publish mods.");
      // Trying again cannot help while the suspension lasts; only Dismiss is offered.
      expect(within(alert).queryByRole('button', { name: 'Try again' })).not.toBeInTheDocument();
      await world.user.click(within(alert).getByRole('button', { name: 'Dismiss' }));
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      expect(world.publisher.stored.size).toBe(0);
    });
  });
});
