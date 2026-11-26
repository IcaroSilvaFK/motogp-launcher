// Binds tests/features/frontend/community-sign-in.feature: the real launcher controller and the real
// UI, with the CommunityAccount port replaced by an in-memory fake. The server and Credential
// Manager halves of these scenarios (revocation, deletion of an expired token) are covered by the
// Rust service tests; here the UI asks the port and shows what it answers.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { describeFeature } from '@amiceli/vitest-cucumber';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, vi } from 'vitest';
import type { MockInstance } from 'vitest';
import { createLauncherController } from '../../application';
import type { CommunityAccount, LauncherController } from '../../application';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { CommunityStatus } from '../../domain';
import { App } from '../App';
import { loadFrontendFeature } from './features';

/** A fake session token the fake backend keeps to itself, the way the real one keeps it in Rust. */
const SESSION_TOKEN = 'mgl_fake_session_7f3c9a1e2b4d6f80';
const AVATAR = 'data:image/png;base64,iVBORw0KGgo=';

/**
 * The CommunityAccount port as the backend behaves: it holds a token internally and only ever
 * answers with a status. `signIn()` stays open until the test authorizes, like the browser round trip.
 */
class FakeCommunityAccount implements CommunityAccount {
  private token: string | null = null;
  private authorize: ((status: CommunityStatus) => void) | null = null;
  readonly signOut = vi.fn(() => {
    this.token = null;
    this.current = { state: 'signed_out' };
    return Promise.resolve();
  });

  constructor(private current: CommunityStatus) {
    if (current.state === 'signed_in') this.token = SESSION_TOKEN;
  }

  status(): Promise<CommunityStatus> {
    return Promise.resolve(this.current);
  }

  signIn(): Promise<CommunityStatus> {
    return new Promise((resolve) => {
      this.authorize = resolve;
    });
  }

  /** The user authorizes in the browser: the backend stores the token and answers the profile. */
  authorizeInBrowser(displayName: string): void {
    this.token = SESSION_TOKEN;
    this.current = { state: 'signed_in', profile: { displayName, avatarDataUrl: AVATAR } };
    this.authorize?.(this.current);
  }

  holdsToken(): boolean {
    return this.token !== null;
  }
}

const game = { name: 'MotoGP 25', root: 'D:/Steam/MotoGP25', mods_dir: 'D:/Steam/MotoGP25/~mods', app_id: 1234 };

interface World {
  account: FakeCommunityAccount;
  launcher: LauncherController;
  user: ReturnType<typeof userEvent.setup>;
}

const CONSOLE_METHODS = ['log', 'info', 'warn', 'error', 'debug'] as const;
let consoleSpies: MockInstance[] = [];

async function launch(status: CommunityStatus): Promise<World> {
  consoleSpies = CONSOLE_METHODS.map((method) => vi.spyOn(console, method).mockImplementation(() => undefined));
  const account = new FakeCommunityAccount(status);
  const launcher = createLauncherController({
    gateway: new FakeGateway({ game }),
    picker: new FakePicker(),
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
  });
  const user = userEvent.setup();
  render(<App controller={launcher} communityAccount={account} locale="en" />);
  await waitFor(() => expect(launcher.getState().status).toBe('ready'));
  return { account, launcher, user };
}

const mainNav = () => within(screen.getByRole('navigation', { name: 'Main navigation' }));

async function openCommunity(world: World) {
  await world.user.click(mainNav().getByRole('button', { name: 'Settings' }));
  const sections = within(screen.getByRole('navigation', { name: 'Settings sections' }));
  await world.user.click(sections.getByRole('link', { name: 'Community' }));
}

const community = () => within(screen.getByRole('region', { name: 'Community' }));

/** Every file under `dir`, recursively. */
function filesUnder(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? filesUnder(path) : [path];
  });
}

// Split so this file does not match its own scan.
const SECRET_KEY = ['DISCORD', 'CLIENT', 'SECRET'].join('_');
/** Values that only stand in for a secret (examples, docs): empty, `<...>`, `changeme`, `placeholder`, `xxx`. */
const PLACEHOLDER = /^(|["']{2}|<[^>]*>|\$\{?\w+\}?|changeme|change-me|placeholder|your[-_].*|x{3,}|\.\.\.)$/i;

const feature = await loadFrontendFeature('community-sign-in');

describeFeature(feature, ({ Scenario, AfterEachScenario }) => {
  AfterEachScenario(() => {
    cleanup();
    for (const spy of consoleSpies) spy.mockRestore();
    consoleSpies = [];
  });

  Scenario('Signing in with Discord', ({ Given, When, Then }) => {
    let world: World;

    Given('the user is signed out', async () => {
      world = await launch({ state: 'signed_out' });
      await openCommunity(world);
      expect(await community().findByRole('button', { name: 'Sign in with Discord' })).toBeEnabled();
    });

    When('they choose "Sign in with Discord" and authorize in the browser', async () => {
      await world.user.click(community().getByRole('button', { name: 'Sign in with Discord' }));
      // While the browser is open the row says so in a live region and the button cannot be pressed twice.
      expect(community().getByRole('status')).toHaveTextContent('Waiting for the browser…');
      expect(community().getByRole('button', { name: 'Sign in with Discord' })).toBeDisabled();
      world.account.authorizeInBrowser('rider');
    });

    Then('the launcher shows their Discord name and avatar', async () => {
      expect(await community().findByText('rider')).toBeInTheDocument();
      const avatar = screen.getByRole('region', { name: 'Community' }).querySelector('img');
      expect(avatar).not.toBeNull();
      expect(avatar).toHaveAttribute('src', AVATAR);
      // Decorative: the name next to it already says who is signed in.
      expect(avatar).toHaveAttribute('alt', '');
      expect(community().getByRole('button', { name: 'Sign out' })).toBeEnabled();
      expect(community().queryByRole('button', { name: 'Sign in with Discord' })).not.toBeInTheDocument();
      expect(community().getByRole('status')).not.toHaveTextContent('Waiting for the browser…');
    });
  });

  Scenario('Signing out revokes the session', ({ Given, When, Then }) => {
    let world: World;

    Given('the user is signed in', async () => {
      world = await launch({ state: 'signed_in', profile: { displayName: 'rider', avatarDataUrl: null } });
      await openCommunity(world);
      expect(await community().findByText('rider')).toBeInTheDocument();
      expect(world.account.holdsToken()).toBe(true);
    });

    When('they sign out', async () => {
      await world.user.click(community().getByRole('button', { name: 'Sign out' }));
    });

    Then('the session token is revoked on the server and removed from the Credential Manager', async () => {
      // The backend revokes and deletes; the UI's part is to ask it once and show the signed-out row.
      expect(world.account.signOut).toHaveBeenCalledTimes(1);
      expect(world.account.holdsToken()).toBe(false);
      expect(await community().findByRole('button', { name: 'Sign in with Discord' })).toBeEnabled();
      expect(community().queryByText('rider')).not.toBeInTheDocument();
    });
  });

  Scenario('An expired session', ({ Given, When, Then }) => {
    let world: World;

    Given('the stored session expired', () => {
      // The server answered 401 to the backend, which deleted the token and reports signed_out.
    });

    When('the launcher starts', async () => {
      world = await launch({ state: 'signed_out' });
      await openCommunity(world);
    });

    Then('the user is shown as signed out without an error', async () => {
      expect(await community().findByRole('button', { name: 'Sign in with Discord' })).toBeEnabled();
      expect(community().queryByRole('alert')).not.toBeInTheDocument();
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
      expect(community().queryByText(/offline/i)).not.toBeInTheDocument();
    });
  });

  Scenario('Server unreachable', ({ Given, Then }) => {
    let world: World;

    Given('the server cannot be reached', async () => {
      world = await launch({ state: 'offline' });
    });

    Then('the rest of the launcher works and the account row says the community is offline', async () => {
      await openCommunity(world);
      expect(await community().findByText(/The community is offline/)).toBeInTheDocument();
      // Neutral, not an error: no alert and no sign-in that would only fail again.
      expect(community().queryByRole('alert')).not.toBeInTheDocument();
      expect(community().queryByRole('button', { name: 'Sign in with Discord' })).not.toBeInTheDocument();
      // The other screens and settings keep working.
      expect(screen.getByRole('region', { name: 'Game' })).toBeInTheDocument();
      await world.user.click(mainNav().getByRole('button', { name: 'Library' }));
      expect(await screen.findByRole('heading', { level: 2, name: 'Library' })).toBeInTheDocument();
    });
  });

  Scenario('Secrets stay out of the repository and logs', ({ Then }) => {
    Then('no client secret or session token appears in the repository, the logs or the frontend', async () => {
      const world = await launch({ state: 'signed_out' });
      await openCommunity(world);
      await world.user.click(await community().findByRole('button', { name: 'Sign in with Discord' }));
      world.account.authorizeInBrowser('rider');
      await community().findByText('rider');
      expect(world.account.holdsToken()).toBe(true);

      // The frontend: the token is never rendered, not even in attributes.
      expect(document.documentElement.outerHTML).not.toContain(SESSION_TOKEN);
      // The logs: nothing printed to the console carries it.
      const printed = consoleSpies.flatMap((spy) => spy.mock.calls.map((call) => call.map(String).join(' ')));
      expect(printed.join('\n')).not.toContain(SESSION_TOKEN);

      // The repository: no real client secret is assigned anywhere in the frontend sources.
      const src = join(import.meta.dirname, '..', '..');
      const assignment = new RegExp(`${SECRET_KEY}=([^\\s'"\`]*)`, 'g');
      const leaks = filesUnder(src).flatMap((file) =>
        [...readFileSync(file, 'utf8').matchAll(assignment)]
          .filter(([, value]) => !PLACEHOLDER.test(value ?? ''))
          .map(([match]) => `${file}: ${match}`),
      );
      expect(leaks).toEqual([]);
    });
  });
});
