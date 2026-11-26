// Binds tests/features/frontend/mod-comments.feature (#118): the real mod page and the real comments
// hook, with the ModComments and CommunityAccount ports replaced by in-memory fakes that behave like
// the backend. The server's half (404 on someone else's comment, the stored cap, Retry-After) is
// covered by the server tests; here the UI shows what the port answers.
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import type { CatalogEntry, CommunityStatus } from '../../domain';
import { CommunityContext } from '../CommunityContext';
import { I18nProvider } from '../i18n';
import { ModPage } from '../ModPage';
import { FakeCommunityAccount, FakeModComments, ME, signedIn, signedOut } from './fakeCommunity';
import { loadFrontendFeature } from './features';

const entry: CatalogEntry = {
  id: 'classic-livery',
  title: 'Classic Livery',
  author: 'Paint Shop',
  game: 'MotoGP 25',
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

interface World {
  account: FakeCommunityAccount;
  comments: FakeModComments;
  user: ReturnType<typeof userEvent.setup>;
}

function setUp(status: CommunityStatus): World {
  const account = new FakeCommunityAccount(status);
  return { account, comments: new FakeModComments(account), user: userEvent.setup() };
}

function openPage(world: World) {
  render(
    <I18nProvider locale="en">
      <CommunityContext.Provider value={{ account: world.account, comments: world.comments }}>
        <ModPage
          entry={entry}
          authorCount={1}
          owned={false}
          pageError={null}
          onOpenPage={vi.fn()}
          onDismissPageError={vi.fn()}
          onBack={vi.fn()}
          onCategory={vi.fn()}
          onAuthor={vi.fn()}
        />
      </CommunityContext.Provider>
    </I18nProvider>,
  );
}

const area = () => within(screen.getByRole('region', { name: 'Comments' }));
const items = () => within(area().getByRole('list')).queryAllByRole('listitem');
const bodies = () => items().map((item) => item.querySelector('.markdown')?.textContent);
const itemWith = (text: string) => {
  const found = items().find((item) => item.querySelector('.markdown')?.textContent === text);
  if (!found) throw new Error(`no comment "${text}"`);
  return within(found);
};

async function post(world: World, text: string) {
  const box = await area().findByRole('textbox', { name: 'Write a comment' });
  await world.user.clear(box);
  if (text.length > 100) fireEvent.change(box, { target: { value: text } });
  else await world.user.type(box, text);
  await world.user.click(area().getByRole('button', { name: 'Post comment' }));
}

const feature = await loadFrontendFeature('mod-comments');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(() => {
    cleanup();
  });

  Scenario('Reading comments without an account', ({ Given, When, Then }) => {
    let world: World;

    Given('a mod has 3 comments', () => {
      world = setUp(signedOut());
      world.comments.add('Ana', 'First!');
      world.comments.add('Bruno Lima', 'Works great with the new season.');
      world.comments.add('Caio', 'Colors look right.');
    });

    When('the user opens its page', () => {
      openPage(world);
    });

    Then('the 3 comments are shown with author, date and text', async () => {
      await waitFor(() => expect(items()).toHaveLength(3));
      expect(area().getByRole('heading', { name: /Comments\s*3/ })).toBeInTheDocument();
      const second = within(items()[1]!);
      expect(second.getByText('Bruno Lima')).toBeInTheDocument();
      expect(second.getByText('Works great with the new season.')).toBeInTheDocument();
      // Relative date in the current locale, the absolute one in the tooltip.
      const time = items()[1]!.querySelector('time');
      expect(time).toHaveTextContent('2 hours ago');
      expect(time?.getAttribute('title')).toBeTruthy();
      expect(time?.getAttribute('dateTime')).toMatch(/^\d{4}-\d{2}-\d{2}T/);
      // Initials without an avatar.
      expect(second.getByText('BL')).toBeInTheDocument();
    });
  });

  Scenario('Posting a comment', ({ Given, When, Then }) => {
    let world: World;

    Given('the user is signed in', () => {
      world = setUp(signedIn());
      world.comments.add('Ana', 'First!');
      openPage(world);
    });

    When('they post "Great livery"', async () => {
      await post(world, 'Great livery');
    });

    Then('the comment appears at the end of the list', async () => {
      await waitFor(() => expect(bodies()).toEqual(['First!', 'Great livery']));
      expect(world.comments.bodies()).toEqual(['First!', 'Great livery']);
      const mine = items()[1]!;
      expect(within(mine).getByText(ME)).toBeInTheDocument();
      expect(within(mine).getByText('now')).toBeInTheDocument();
      // The composer is cleared for the next one.
      expect(area().getByRole('textbox', { name: 'Write a comment' })).toHaveValue('');
    });
  });

  Scenario('Offline', ({ Given, Then }) => {
    let world: World;

    Given('the server cannot be reached', () => {
      world = setUp(signedIn());
      world.comments.offline = true;
      openPage(world);
    });

    Then('the page works and the comments area shows a quiet message', async () => {
      expect(await area().findByText(/Comments are unavailable while the community is offline/)).toBeInTheDocument();
      // Quiet: a status, not an alert, and no composer that would only fail.
      expect(area().queryByRole('alert')).not.toBeInTheDocument();
      expect(area().queryByRole('textbox')).not.toBeInTheDocument();
      // The rest of the page keeps working.
      expect(screen.getByRole('heading', { level: 2, name: 'Classic Livery' })).toBeInTheDocument();
      expect(screen.getByText('A classic livery pack.')).toBeInTheDocument();
      // Retry reads again once the server is back.
      world.comments.offline = false;
      world.comments.add('Ana', 'Back online');
      await world.user.click(area().getByRole('button', { name: 'Try again' }));
      expect(await area().findByText('Back online')).toBeInTheDocument();
    });
  });

  Scenario('Signed out users are invited to sign in', ({ Given, When, Then }) => {
    let world: World;

    Given('the user is not signed in', () => {
      world = setUp(signedOut());
    });

    When('they open a mod page', () => {
      openPage(world);
    });

    Then('they see "Sign in with Discord to comment" instead of the composer', async () => {
      const signIn = await area().findByRole('button', { name: 'Sign in with Discord to comment' });
      expect(area().queryByRole('textbox')).not.toBeInTheDocument();
      // It runs the existing Discord sign-in; the composer takes its place once signed in.
      await world.user.click(signIn);
      expect(world.account.signIn).toHaveBeenCalledTimes(1);
      expect(await area().findByRole('textbox', { name: 'Write a comment' })).toBeInTheDocument();
      expect(area().queryByRole('button', { name: 'Sign in with Discord to comment' })).not.toBeInTheDocument();
    });
  });

  Scenario('Loading more comments', ({ Given, When, Then }) => {
    let world: World;

    Given('a mod has 25 comments', () => {
      world = setUp(signedOut());
      for (let n = 1; n <= 25; n += 1) world.comments.add('Ana', `Comment ${n}`);
    });

    When('the user opens its page', () => {
      openPage(world);
    });

    Then('20 comments are shown', async () => {
      await waitFor(() => expect(items()).toHaveLength(20));
      expect(area().getByRole('heading', { name: /Comments\s*20\+/ })).toBeInTheDocument();
    });

    When('they press "Load more"', async () => {
      await world.user.click(area().getByRole('button', { name: 'Load more' }));
    });

    Then('25 comments are shown in posting order', async () => {
      await waitFor(() => expect(items()).toHaveLength(25));
      expect(bodies()).toEqual(Array.from({ length: 25 }, (_, i) => `Comment ${i + 1}`));
      expect(area().queryByRole('button', { name: 'Load more' })).not.toBeInTheDocument();
    });
  });

  Scenario('Editing my own comment', ({ Given, When, Then }) => {
    let world: World;

    Given('the user posted "Nice"', async () => {
      world = setUp(signedIn());
      world.comments.add(ME, 'Nice');
      openPage(world);
      await waitFor(() => expect(bodies()).toEqual(['Nice']));
    });

    When('they edit it to "Nice livery"', async () => {
      await world.user.click(itemWith('Nice').getByRole('button', { name: 'Edit' }));
      const box = area().getByRole('textbox', { name: 'Edit your comment' });
      expect(box).toHaveValue('Nice');
      expect(box).toHaveFocus();
      await world.user.clear(box);
      await world.user.type(box, 'Nice livery');
      await world.user.click(area().getByRole('button', { name: 'Save' }));
    });

    Then('the comment shows "Nice livery" and is marked edited', async () => {
      await waitFor(() => expect(bodies()).toEqual(['Nice livery']));
      expect(within(items()[0]!).getByText('edited')).toBeInTheDocument();
      expect(area().queryByRole('textbox', { name: 'Edit your comment' })).not.toBeInTheDocument();
      expect(world.comments.bodies()).toEqual(['Nice livery']);
    });
  });

  Scenario('Deleting my own comment', ({ Given, When, Then }) => {
    let world: World;

    Given('the user posted "Oops"', async () => {
      world = setUp(signedIn());
      world.comments.add('Ana', 'Keep me');
      world.comments.add(ME, 'Oops');
      openPage(world);
      await waitFor(() => expect(bodies()).toEqual(['Keep me', 'Oops']));
    });

    When('they delete it and confirm', async () => {
      await world.user.click(itemWith('Oops').getByRole('button', { name: 'Delete' }));
      // A confirmation step first: nothing is deleted yet.
      const confirm = itemWith('Oops').getByRole('group', { name: 'Delete this comment? This cannot be undone.' });
      expect(world.comments.remove).not.toHaveBeenCalled();
      await world.user.click(within(confirm).getByRole('button', { name: 'Delete' }));
    });

    Then('the comment is gone', async () => {
      await waitFor(() => expect(bodies()).toEqual(['Keep me']));
      expect(world.comments.bodies()).toEqual(['Keep me']);
    });
  });

  Scenario("Someone else's comment cannot be changed", ({ Given, Then }) => {
    let world: World;

    Given('another user posted a comment', async () => {
      world = setUp(signedIn());
      world.comments.add('Ana', 'Not yours');
      world.comments.add(ME, 'Mine');
      openPage(world);
      await waitFor(() => expect(items()).toHaveLength(2));
    });

    Then('it has no edit or delete actions', () => {
      expect(itemWith('Not yours').queryByRole('button')).not.toBeInTheDocument();
      expect(itemWith('Mine').getByRole('button', { name: 'Edit' })).toBeInTheDocument();
      expect(itemWith('Mine').getByRole('button', { name: 'Delete' })).toBeInTheDocument();
    });
  });

  Scenario('Comment length cap', ({ When, Then }) => {
    let world: World;

    When('the user tries to post 2001 characters', async () => {
      world = setUp(signedIn());
      openPage(world);
      const box = await area().findByRole('textbox', { name: 'Write a comment' });
      fireEvent.change(box, { target: { value: 'a'.repeat(2001) } });
      // Ctrl+Enter is the keyboard way to post; it must not get around the cap.
      fireEvent.keyDown(box, { key: 'Enter', ctrlKey: true });
    });

    Then('the comment is rejected and nothing is stored', () => {
      expect(area().getByRole('button', { name: 'Post comment' })).toBeDisabled();
      expect(area().getByText('2001 / 2000')).toBeInTheDocument();
      expect(area().getByText('Too long by 1 character.')).toBeInTheDocument();
      expect(world.comments.post).not.toHaveBeenCalled();
      expect(world.comments.bodies()).toEqual([]);
    });
  });

  Scenario('Per-user write limit', ({ Given, When, Then }) => {
    let world: World;

    Given('the user posted 5 comments in the last minute', async () => {
      world = setUp(signedIn());
      world.comments.writeLimit = 5;
      openPage(world);
      for (let n = 1; n <= 5; n += 1) {
        await post(world, `Post ${n}`);
        await waitFor(() => expect(items()).toHaveLength(n));
      }
    });

    When('they post another one', async () => {
      await post(world, 'One more');
    });

    Then('the launcher shows a "slow down" message', async () => {
      const alert = await area().findByRole('alert');
      expect(alert).toHaveTextContent('Slow down: you can comment again in 42 seconds.');
      expect(world.comments.bodies()).toHaveLength(5);
      // The draft is kept so nothing typed is lost.
      expect(area().getByRole('textbox', { name: 'Write a comment' })).toHaveValue('One more');
    });
  });

  Scenario('No raw HTML', ({ Given, Then }) => {
    const raw = '<img src=x onerror=alert(1)>';

    Given('a comment body "<img src=x onerror=alert(1)>"', () => {
      const world = setUp(signedOut());
      world.comments.add('Ana', raw);
      openPage(world);
    });

    Then('it is shown as text, not as an element', async () => {
      expect(await area().findByText(raw)).toBeInTheDocument();
      const region = screen.getByRole('region', { name: 'Comments' });
      expect(region.querySelector('[onerror]')).toBeNull();
      expect(region.querySelectorAll('img')).toHaveLength(0);
    });
  });
});
