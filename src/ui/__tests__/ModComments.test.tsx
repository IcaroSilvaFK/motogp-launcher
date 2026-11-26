import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CommunityStatus } from '../../domain';
import { CommunityContext } from '../CommunityContext';
import type { CommunityContextValue } from '../CommunityContext';
import { I18nProvider } from '../i18n';
import type { Locale } from '../i18n';
import { formatRelativeTime, ModComments } from '../ModComments';
import { FakeCommunityAccount, FakeModComments, ME, signedIn, signedOut } from './fakeCommunity';

const AVATAR = 'data:image/png;base64,iVBORw0KGgo=';

function renderComments(value: CommunityContextValue, locale: Locale = 'en', onKeyDown = vi.fn()) {
  const view = render(
    <I18nProvider locale={locale}>
      <CommunityContext.Provider value={value}>
        <div onKeyDown={onKeyDown}>
          <ModComments modId="classic-livery" />
        </div>
      </CommunityContext.Provider>
    </I18nProvider>,
  );
  return { ...view, onKeyDown };
}

function setUp(status: CommunityStatus = signedIn()) {
  const account = new FakeCommunityAccount(status);
  const comments = new FakeModComments(account);
  return { account, comments, user: userEvent.setup() };
}

const area = (name = 'Comments') => within(screen.getByRole('region', { name }));

describe('ModComments', () => {
  afterEach(cleanup);

  it('renders nothing without a comments port', () => {
    const { container } = renderComments({ account: new FakeCommunityAccount(signedIn()) });
    expect(container.firstElementChild).toBeEmptyDOMElement();
  });

  it('shows a busy skeleton while the first page loads, then the empty state', async () => {
    const { account, comments } = setUp();
    comments.holdLists();
    renderComments({ account, comments });
    const region = screen.getByRole('region', { name: 'Comments' });
    expect(region.querySelector('[aria-busy="true"] .skeleton')).not.toBeNull();
    expect(area().getByText('Loading comments…')).toBeInTheDocument();
    comments.releaseLists();
    expect(await area().findByText('No comments yet. Be the first.')).toBeInTheDocument();
    expect(region.querySelector('[aria-busy="true"]')).toBeNull();
    expect(area().getByRole('heading', { name: /Comments\s*0/ })).toBeInTheDocument();
  });

  it('shows the avatar image when the author has one, decorative', async () => {
    const { account, comments } = setUp(signedOut());
    comments.add('Ana', 'Hi', Date.now() - 60_000, AVATAR);
    renderComments({ account, comments });
    await area().findByText('Hi');
    const img = screen.getByRole('region', { name: 'Comments' }).querySelector('img');
    expect(img).toHaveAttribute('src', AVATAR);
    expect(img).toHaveAttribute('alt', '');
  });

  it('marks the member own comments with a "You" chip', async () => {
    const { account, comments } = setUp();
    comments.add(ME, 'Mine');
    comments.add('Ana', 'Theirs');
    renderComments({ account, comments });
    await area().findByText('Mine');
    const [mine, theirs] = within(area().getByRole('list')).getAllByRole('listitem');
    expect(within(mine!).getByText('You')).toBeInTheDocument();
    expect(within(theirs!).queryByText('You')).not.toBeInTheDocument();
  });

  it('keeps the list read-only without an account port', async () => {
    const { comments } = setUp();
    comments.add('Ana', 'Hi');
    renderComments({ comments });
    await area().findByText('Hi');
    expect(area().queryByRole('textbox')).not.toBeInTheDocument();
    expect(area().queryByRole('button', { name: /Sign in/ })).not.toBeInTheDocument();
  });

  it('says posting is unavailable while the account reports offline', async () => {
    const { account, comments } = setUp({ state: 'offline' });
    renderComments({ account, comments });
    expect(await area().findByText('Posting is unavailable while the community is offline.')).toBeInTheDocument();
    expect(area().queryByRole('textbox')).not.toBeInTheDocument();
  });

  it('counts characters live and disables Post while empty', async () => {
    const { account, comments, user } = setUp();
    renderComments({ account, comments });
    const box = await area().findByRole('textbox', { name: 'Write a comment' });
    const postButton = area().getByRole('button', { name: 'Post comment' });
    expect(postButton).toBeDisabled();
    expect(area().getByText('0 / 2000')).toBeInTheDocument();
    await user.type(box, 'Hello');
    expect(area().getByText('5 / 2000')).toBeInTheDocument();
    expect(postButton).toBeEnabled();
    // The counter is read with the field, not announced on every key.
    expect(box.getAttribute('aria-describedby')).toContain(area().getByText('5 / 2000').id);
  });

  it('posts with Ctrl+Enter', async () => {
    const { account, comments, user } = setUp();
    renderComments({ account, comments });
    const box = await area().findByRole('textbox', { name: 'Write a comment' });
    await user.type(box, 'Quick one');
    fireEvent.keyDown(box, { key: 'Enter', ctrlKey: true });
    expect(await area().findByText('Quick one', { selector: '.markdown p' })).toBeInTheDocument();
    expect(comments.post).toHaveBeenCalledWith('classic-livery', 'Quick one');
  });

  it('offers sign-in again when a write finds the session gone', async () => {
    const { account, comments, user } = setUp();
    renderComments({ account, comments });
    const box = await area().findByRole('textbox', { name: 'Write a comment' });
    account.expire();
    await user.type(box, 'Hello');
    await user.click(area().getByRole('button', { name: 'Post comment' }));
    expect(await area().findByRole('alert')).toHaveTextContent('Your session ended.');
    const signIn = area().getByRole('button', { name: 'Sign in with Discord to comment' });
    expect(area().queryByRole('textbox')).not.toBeInTheDocument();
    await user.click(signIn);
    expect(account.signIn).toHaveBeenCalledTimes(1);
    // Signed in again: the composer comes back and the error is gone.
    expect(await area().findByRole('textbox', { name: 'Write a comment' })).toBeInTheDocument();
    expect(area().queryByRole('alert')).not.toBeInTheDocument();
  });

  it('shows a dismissible error when a comment is already gone', async () => {
    const { account, comments, user } = setUp();
    const stored = comments.add(ME, 'Mine');
    renderComments({ account, comments });
    await area().findByText('Mine');
    // Someone removed it on the server meanwhile.
    stored.author = 'someone else';
    await user.click(area().getByRole('button', { name: 'Delete' }));
    await user.click(within(area().getByRole('group')).getByRole('button', { name: 'Delete' }));
    expect(await area().findByRole('alert')).toHaveTextContent('That comment no longer exists.');
    expect(area().queryByText('Mine')).not.toBeInTheDocument();
    await user.click(area().getByRole('button', { name: 'Dismiss' }));
    expect(area().queryByRole('alert')).not.toBeInTheDocument();
  });

  it('cancels an edit with Escape without leaving the page', async () => {
    const { account, comments, user } = setUp();
    comments.add(ME, 'Nice');
    const { onKeyDown } = renderComments({ account, comments });
    await area().findByText('Nice');
    const editButton = area().getByRole('button', { name: 'Edit' });
    await user.click(editButton);
    const box = area().getByRole('textbox', { name: 'Edit your comment' });
    await user.type(box, ' and more');
    await user.keyboard('{Escape}');
    expect(area().queryByRole('textbox', { name: 'Edit your comment' })).not.toBeInTheDocument();
    expect(area().getByText('Nice')).toBeInTheDocument();
    expect(comments.edit).not.toHaveBeenCalled();
    // The page's Esc (back to the list) sees the event already handled.
    expect(onKeyDown.mock.calls.at(-1)?.[0].defaultPrevented).toBe(true);
    await waitFor(() => expect(area().getByRole('button', { name: 'Edit' })).toHaveFocus());
  });

  it('keeps Save disabled while the edit is unchanged or empty', async () => {
    const { account, comments, user } = setUp();
    comments.add(ME, 'Nice');
    renderComments({ account, comments });
    await area().findByText('Nice');
    await user.click(area().getByRole('button', { name: 'Edit' }));
    const save = area().getByRole('button', { name: 'Save' });
    expect(save).toBeDisabled();
    await user.clear(area().getByRole('textbox', { name: 'Edit your comment' }));
    expect(save).toBeDisabled();
  });

  it('cancels a delete confirmation and restores the actions', async () => {
    const { account, comments, user } = setUp();
    comments.add(ME, 'Mine');
    renderComments({ account, comments });
    await area().findByText('Mine');
    await user.click(area().getByRole('button', { name: 'Delete' }));
    const group = area().getByRole('group');
    expect(within(group).getByRole('button', { name: 'Cancel' })).toHaveFocus();
    await user.click(within(group).getByRole('button', { name: 'Cancel' }));
    expect(area().queryByRole('group')).not.toBeInTheDocument();
    expect(comments.remove).not.toHaveBeenCalled();
    expect(area().getByText('Mine')).toBeInTheDocument();
  });

  it('is translated to Brazilian Portuguese', async () => {
    const { account, comments } = setUp(signedOut());
    comments.add('Ana', 'Oi');
    renderComments({ account, comments }, 'pt-BR');
    await area('Comentários').findByText('Oi');
    expect(area('Comentários').getByText('há 2 horas')).toBeInTheDocument();
    expect(area('Comentários').getByRole('button', { name: 'Entre com o Discord para comentar' })).toBeInTheDocument();
  });
});

describe('formatRelativeTime', () => {
  const now = Date.UTC(2026, 10, 26, 12);
  const ago = (seconds: number) => formatRelativeTime(now - seconds * 1000, 'en', now);

  it('picks the largest whole unit', () => {
    expect(ago(10)).toBe('now');
    expect(ago(90)).toBe('1 minute ago');
    expect(ago(3 * 3600)).toBe('3 hours ago');
    expect(ago(86400)).toBe('yesterday');
    expect(ago(8 * 86400)).toBe('last week');
    expect(ago(40 * 86400)).toBe('last month');
    expect(ago(400 * 86400)).toBe('last year');
  });

  it('never shows a time in the future when the clocks disagree', () => {
    expect(formatRelativeTime(now + 60_000, 'en', now)).toBe('now');
  });
});
