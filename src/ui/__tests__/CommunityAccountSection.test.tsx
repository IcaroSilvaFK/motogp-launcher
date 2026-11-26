import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CommunityAccount } from '../../application';
import { CommunitySignInError } from '../../domain';
import type { CommunityErrorCode, CommunityStatus } from '../../domain';
import { CommunityAccountSection, initialsOf } from '../CommunityAccountSection';
import { CommunityContext } from '../CommunityContext';
import { I18nProvider } from '../i18n';
import type { Locale } from '../i18n';

function port(overrides: Partial<CommunityAccount> = {}): CommunityAccount {
  return {
    status: vi.fn(() => Promise.resolve<CommunityStatus>({ state: 'signed_out' })),
    signIn: vi.fn(() => Promise.resolve<CommunityStatus>({ state: 'signed_out' })),
    signOut: vi.fn(() => Promise.resolve()),
    ...overrides,
  };
}

function renderSection(account: CommunityAccount | undefined, locale: Locale = 'en') {
  return render(
    <I18nProvider locale={locale}>
      <CommunityContext.Provider value={{ account }}>
        <CommunityAccountSection />
      </CommunityContext.Provider>
    </I18nProvider>,
  );
}

const section = (name = 'Community') => within(screen.getByRole('region', { name }));

describe('CommunityAccountSection', () => {
  afterEach(cleanup);

  it('renders nothing without a port', () => {
    const { container } = renderSection(undefined);
    expect(container).toBeEmptyDOMElement();
  });

  it('holds the row with a busy placeholder until the first status answer, with no wrong state', () => {
    renderSection(port({ status: () => new Promise<CommunityStatus>(() => {}) }));
    const card = screen.getByRole('region', { name: 'Community' }).querySelector('.card') as HTMLElement;
    expect(card).toHaveAttribute('aria-busy', 'true');
    expect(card.querySelector('.skeleton')).not.toBeNull();
    expect(section().queryByRole('button')).not.toBeInTheDocument();
    expect(section().queryByText(/offline/i)).not.toBeInTheDocument();
  });

  it('shows initials when the member has no avatar', async () => {
    renderSection(
      port({
        status: () =>
          Promise.resolve({ state: 'signed_in', profile: { displayName: 'Ana Souza', avatarDataUrl: null } }),
      }),
    );
    expect(await section().findByText('Ana Souza')).toBeInTheDocument();
    const region = screen.getByRole('region', { name: 'Community' });
    expect(region.querySelector('img')).toBeNull();
    const initials = region.querySelector('.account-avatar') as HTMLElement;
    expect(initials).toHaveTextContent('AS');
    expect(initials).toHaveAttribute('aria-hidden', 'true');
  });

  it.each<[CommunityErrorCode, string]>([
    ['denied', 'Sign-in was cancelled in the browser.'],
    ['timeout', 'The browser did not answer in time. Try again.'],
    ['failed', 'Could not sign in. Try again.'],
  ])('explains a %s sign-in and lets the user dismiss it', async (code, message) => {
    const user = userEvent.setup();
    renderSection(port({ signIn: () => Promise.reject(new CommunitySignInError(code)) }));
    await user.click(await section().findByRole('button', { name: 'Sign in with Discord' }));
    const alert = await section().findByRole('alert');
    expect(alert).toHaveTextContent(message);
    // The user can try again right away.
    expect(section().getByRole('button', { name: 'Sign in with Discord' })).toBeEnabled();
    await user.click(within(alert).getByRole('button', { name: 'Dismiss' }));
    expect(section().queryByRole('alert')).not.toBeInTheDocument();
  });

  it('clears the error on the next attempt', async () => {
    const user = userEvent.setup();
    const signIn = vi
      .fn<CommunityAccount['signIn']>()
      .mockRejectedValueOnce(new CommunitySignInError('timeout'))
      .mockReturnValueOnce(new Promise<CommunityStatus>(() => {}));
    renderSection(port({ signIn }));
    await user.click(await section().findByRole('button', { name: 'Sign in with Discord' }));
    expect(await section().findByRole('alert')).toBeInTheDocument();
    await user.click(section().getByRole('button', { name: 'Sign in with Discord' }));
    expect(section().queryByRole('alert')).not.toBeInTheDocument();
    expect(section().getByRole('status')).toHaveTextContent('Waiting for the browser…');
  });

  it('shows an offline sign-in as the neutral offline message, not an error', async () => {
    const user = userEvent.setup();
    renderSection(port({ signIn: () => Promise.reject(new CommunitySignInError('offline')) }));
    await user.click(await section().findByRole('button', { name: 'Sign in with Discord' }));
    expect(await section().findByText(/The community is offline/)).toBeInTheDocument();
    expect(section().queryByRole('alert')).not.toBeInTheDocument();
  });

  it('is translated to pt-BR', async () => {
    renderSection(port(), 'pt-BR');
    expect(await section('Comunidade').findByRole('button', { name: 'Entrar com o Discord' })).toBeEnabled();
  });
});

describe('initialsOf', () => {
  it.each([
    ['rider', 'R'],
    ['Ana Souza', 'AS'],
    ['  marc   márquez  93 ', 'MM'],
    // Non-ASCII on purpose: Discord names often start with accents or emoji.
    ['Élio', 'É'],
    ['😀 fan', '😀F'],
    ['', '?'],
  ])('%j → %j', (name, initials) => {
    expect(initialsOf(name)).toBe(initials);
  });
});
