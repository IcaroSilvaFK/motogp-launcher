import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createModStatsStore } from '../../application';
import type { CommunityAccount, ModStatsSource } from '../../application';
import { ModStatsError } from '../../domain';
import type { CommunityStatus } from '../../domain';
import { CommunityContext } from '../CommunityContext';
import { I18nProvider } from '../i18n';
import { StarButton } from '../StarButton';

const SIGNED_IN: CommunityStatus = { state: 'signed_in', profile: { displayName: 'rider', avatarDataUrl: null } };

function account(status: CommunityStatus): CommunityAccount {
  return {
    status: () => Promise.resolve(status),
    signIn: vi.fn(() => Promise.reject(Object.assign(new Error('denied'), { code: 'community_sign_in_denied' }))),
    signOut: () => Promise.resolve(),
  };
}

function source(patch: Partial<ModStatsSource> = {}): ModStatsSource {
  return {
    stats: () => Promise.resolve([{ id: 'a', stars: 3 }]),
    star: (id) => Promise.resolve({ id, stars: 4, starredByMe: true }),
    unstar: (id) => Promise.resolve({ id, stars: 3, starredByMe: false }),
    ...patch,
  };
}

function renderStar(stats: ModStatsSource, communityAccount?: CommunityAccount) {
  const stars = createModStatsStore(stats);
  render(
    <I18nProvider locale="en">
      <CommunityContext.Provider value={{ account: communityAccount, stars }}>
        <StarButton modId="a" />
      </CommunityContext.Provider>
    </I18nProvider>,
  );
}

afterEach(cleanup);

describe('StarButton', () => {
  it('renders nothing without a stats store', () => {
    render(
      <I18nProvider locale="en">
        <StarButton modId="a" />
      </I18nProvider>,
    );
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('reverts and says so when the star fails', async () => {
    const user = userEvent.setup();
    renderStar(source({ star: () => Promise.reject(new ModStatsError('offline')) }), account(SIGNED_IN));
    await user.click(await screen.findByRole('button', { name: 'Star' }));
    expect(await screen.findByText('Could not update the star. Try again.')).toBeInTheDocument();
    expect(screen.getByText('3 stars')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Star' })).toHaveAttribute('aria-pressed', 'false');
  });

  it('shows the counts without an account port and reports a refused star as a failure', async () => {
    const user = userEvent.setup();
    renderStar(source({ star: () => Promise.reject(new ModStatsError('signedOut')) }));
    await user.click(await screen.findByRole('button', { name: 'Star' }));
    expect(await screen.findByText('Could not update the star. Try again.')).toBeInTheDocument();
    expect(screen.queryByRole('group')).not.toBeInTheDocument();
  });

  it('offers sign-in when the server says the session is gone', async () => {
    const user = userEvent.setup();
    renderStar(source({ star: () => Promise.reject(new ModStatsError('signedOut')) }), account(SIGNED_IN));
    await user.click(await screen.findByRole('button', { name: 'Star' }));
    expect(await screen.findByRole('group', { name: 'Sign in with Discord to star mods.' })).toBeInTheDocument();
  });

  it('"Not now" closes the prompt and a failed sign-in is shown in it', async () => {
    const user = userEvent.setup();
    const signedOut = account({ state: 'signed_out' });
    renderStar(source(), signedOut);
    await user.click(await screen.findByRole('button', { name: 'Star' }));
    let prompt = screen.getByRole('group', { name: 'Sign in with Discord to star mods.' });
    await user.click(within(prompt).getByRole('button', { name: 'Sign in with Discord' }));
    expect(await within(prompt).findByText('Sign-in was cancelled in the browser.')).toBeInTheDocument();
    await user.click(within(prompt).getByRole('button', { name: 'Not now' }));
    expect(screen.queryByRole('group')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Star' })).toHaveFocus();
    await user.click(screen.getByRole('button', { name: 'Star' }));
    prompt = screen.getByRole('group', { name: 'Sign in with Discord to star mods.' });
    expect(prompt).toBeInTheDocument();
  });

  it('hides the button while the community is offline and no stats came back', async () => {
    renderStar(source({ stats: () => Promise.resolve([]) }), account({ state: 'offline' }));
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.queryByRole('button', { name: 'Star' })).not.toBeInTheDocument();
  });
});
