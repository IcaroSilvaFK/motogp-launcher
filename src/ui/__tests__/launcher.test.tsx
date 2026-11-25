// Frontend scenarios from tests/features/frontend/interface.feature, against a fake controller.
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import type { GameInfo, ModEntry } from '../../domain';
import { App } from '../App';
import { createFakeController } from './fakeController';

const fakeGame: GameInfo = {
  name: 'MotoGPFake',
  root: 'C:/temp/MotoGPFake',
  mods_dir: 'C:/temp/MotoGPFake/MotoGPFake/Content/Paks/~mods',
  app_id: null,
};
const steamGame: GameInfo = {
  name: 'MotoGP 25',
  root: 'D:/Steam/MotoGP25',
  mods_dir: 'D:/Steam/MotoGP25/~mods',
  app_id: 1234,
};
const modA: ModEntry = { id: 'A', name: 'Mod A', files: ['a.pak', 'a.utoc'], size_bytes: 2048 };
const modB: ModEntry = { id: 'B', name: 'Mod B', files: ['b.pak'], size_bytes: 500 };

afterEach(cleanup);

const renderApp = (controller = createFakeController()) => {
  const user = userEvent.setup();
  const view = render(<App controller={controller} locale="en" />);
  return { user, controller, ...view };
};

const openScreen = (user: ReturnType<typeof userEvent.setup>, name: string) =>
  user.click(within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', { name }));

describe('Given the Settings screen', () => {
  it('settings_accepts_fake_unreal_directory: When a fake Unreal folder is configured Then I see mods_dir and App ID not found', async () => {
    const { user, controller } = renderApp(createFakeController({ game: fakeGame }));
    await openScreen(user, 'Settings');

    // The Settings details; the status bar also names the game.
    expect(within(screen.getByRole('main')).getByText('MotoGPFake')).toBeInTheDocument();
    expect(screen.getByText(fakeGame.mods_dir)).toBeInTheDocument();
    expect(screen.getByText('not found')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Choose folder…' }));
    expect(controller.chooseGameDirectory).toHaveBeenCalledTimes(1);
  });

  it('selects_detected_game: When I click Use this Then the game is configured', async () => {
    const { user, controller } = renderApp(createFakeController({ detected: [steamGame] }));
    await openScreen(user, 'Settings');

    await user.click(screen.getByRole('button', { name: 'Use this' }));
    expect(controller.useGame).toHaveBeenCalledWith('D:/Steam/MotoGP25');
  });

  it('detect_button_runs_detection: When I click Detect games Then detection runs', async () => {
    const { user, controller } = renderApp();
    await openScreen(user, 'Settings');

    await user.click(screen.getByRole('button', { name: 'Detect games' }));
    expect(controller.detectGames).toHaveBeenCalledTimes(1);
  });

  it('language_select_switches_locale: When I pick Portuguese Then labels change', async () => {
    const { user } = renderApp();
    await openScreen(user, 'Settings');

    await user.selectOptions(screen.getByLabelText('Language'), 'pt-BR');
    expect(screen.getByRole('navigation', { name: 'Navegação principal' })).toBeInTheDocument();
  });
});

describe('Given the Library screen', () => {
  it('library_shows_mod_metadata: When mods exist Then I see name, size and file count and can remove', async () => {
    const { user, controller } = renderApp(createFakeController({ mods: [modA, modB] }));

    const row = screen.getByText('Mod A').closest('li') as HTMLElement;
    expect(within(row).getByText('2 KB')).toBeInTheDocument();
    expect(within(row).getByText('2 files')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Remove Mod A' }));
    expect(controller.removeMod).toHaveBeenCalledWith('A');
  });

  it('imports_from_dialog_and_drop: When I click Import Then the dialog flow runs and drop is subscribed', async () => {
    const { user, controller } = renderApp();

    await user.click(screen.getByRole('button', { name: 'Import mods' }));
    expect(controller.importFromDialog).toHaveBeenCalledTimes(1);
    expect(controller.counters.subscribeDrop).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/Drop \.pak/)).toBeInTheDocument();
  });

  it('cancelled_dialog_does_not_import: When I unmount Then the drop listener is released once and dispose is called once', async () => {
    const { user, controller, unmount } = renderApp();

    await user.click(screen.getByRole('button', { name: 'Import mods' }));
    expect(controller.importPaths).not.toHaveBeenCalled();
    await act(async () => {
      await Promise.resolve();
    });
    unmount();
    await act(async () => {
      await Promise.resolve();
    });
    expect(controller.counters.subscribeDrop).toBe(1);
    expect(controller.release).toHaveBeenCalledTimes(1);
    expect(controller.dispose).toHaveBeenCalledTimes(1);
    expect(controller.start).toHaveBeenCalledTimes(1);
  });

  it('row_switch_is_inert_while_busy: When the controller is busy Then the switch ignores clicks without a hint', async () => {
    const { user, controller } = renderApp(
      createFakeController({
        mods: [modA],
        profiles: [
          { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
          { id: 'p1', name: 'Mine', mod_ids: [] },
        ],
        config: { game_dir: null, active_profile: 'p1' },
        busy: true,
      }),
    );

    const toggle = screen.getByRole('switch', { name: 'Include Mod A in Mine' });
    expect(toggle).toHaveAttribute('aria-disabled', 'true');
    expect(toggle).not.toHaveAttribute('title');
    expect(toggle).not.toHaveAttribute('aria-describedby');
    await user.click(toggle);
    expect(controller.toggleModInActiveProfile).not.toHaveBeenCalled();

    act(() => controller.setState({ busy: false }));
    expect(toggle).not.toHaveAttribute('aria-disabled');
    await user.click(toggle);
    expect(controller.toggleModInActiveProfile).toHaveBeenCalledWith('A');
  });

  it('row_switch_needs_a_profile: When no profile is active Then the switch asks to select one', async () => {
    const { user, controller } = renderApp(createFakeController({ mods: [modA] }));

    const toggle = screen.getByRole('switch', { name: 'Include Mod A in the active profile' });
    expect(toggle).toHaveAttribute('aria-disabled', 'true');
    expect(toggle).toHaveAttribute('title', 'Select a profile first.');
    expect(toggle).toHaveAccessibleDescription('Select a profile first.');
    await user.click(toggle);
    expect(controller.toggleModInActiveProfile).not.toHaveBeenCalled();
  });
});

describe('Given the Profiles screen', () => {
  const profiles = [
    { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
    { id: 'p1', name: 'Teste', mod_ids: [] },
  ];

  it('creates_renames_and_deletes_profile: When I create, rename and delete Then the controller is called', async () => {
    const { user, controller } = renderApp(createFakeController({ profiles }));
    await openScreen(user, 'Profiles');

    await user.type(screen.getByLabelText('Profile name'), 'Novo');
    await user.click(screen.getByRole('button', { name: 'Create profile' }));
    expect(controller.createProfile).toHaveBeenCalledWith('Novo');

    await user.click(screen.getByRole('button', { name: 'Teste' }));
    const rename = screen.getByLabelText('New name');
    await user.clear(rename);
    await user.type(rename, 'Renamed');
    await user.click(screen.getByRole('button', { name: 'Rename' }));
    expect(controller.renameProfile).toHaveBeenCalledWith('p1', 'Renamed');

    await user.click(screen.getByRole('button', { name: 'Delete Teste' }));
    expect(controller.deleteProfile).toHaveBeenCalledWith('p1');
  });

  it('vanilla_delete_is_unavailable: When Vanilla is selected Then there is no delete action', async () => {
    const { user } = renderApp(createFakeController({ profiles }));
    await openScreen(user, 'Profiles');

    await user.click(screen.getByRole('button', { name: 'Vanilla' }));
    expect(screen.queryByRole('button', { name: /^Delete/ })).not.toBeInTheDocument();
    expect(screen.getByText('Vanilla cannot be deleted.')).toBeInTheDocument();
  });

  it('profile_order_can_be_changed_with_keyboard: When I check A and B and move B up Then save sends B, A', async () => {
    const { user, controller } = renderApp(createFakeController({ profiles, mods: [modA, modB] }));
    await openScreen(user, 'Profiles');
    await user.click(screen.getByRole('button', { name: 'Teste' }));

    await user.click(screen.getByRole('checkbox', { name: 'Mod A' }));
    await user.click(screen.getByRole('checkbox', { name: 'Mod B' }));
    expect(screen.getByRole('button', { name: 'Move Mod A up' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Move Mod B down' })).toBeDisabled();

    // Keyboard only from here.
    act(() => screen.getByRole('button', { name: 'Move Mod B up' }).focus());
    await user.keyboard('{Enter}');
    const ordered = screen
      .getAllByRole('listitem')
      .filter((li) => within(li).queryAllByRole('button', { name: /^Move/ }).length > 0);
    expect(ordered.map((li) => li.textContent)).toEqual([
      expect.stringContaining('Mod B'),
      expect.stringContaining('Mod A'),
    ]);
    act(() => screen.getByRole('button', { name: 'Save' }).focus());
    await user.keyboard(' ');
    expect(controller.setProfileMods).toHaveBeenCalledWith('p1', ['B', 'A']);
  });
});

describe('Given the shell', () => {
  it('play_is_disabled_without_game_or_during_session: When no game or running Then Play is disabled', () => {
    const controller = createFakeController();
    renderApp(controller);
    expect(screen.getByRole('button', { name: 'Play' })).toBeDisabled();

    act(() =>
      controller.setState({ game: steamGame, config: { game_dir: steamGame.root, active_profile: 'vanilla' } }),
    );
    expect(screen.getByRole('button', { name: 'Play' })).toBeEnabled();

    // While a session can be stopped, Stop takes Play's place (#75): Play is not offered at all.
    act(() => controller.setState({ session: { kind: 'starting' } }));
    expect(screen.queryByRole('button', { name: 'Play' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Stop' })).toBeEnabled();
    act(() => controller.setState({ session: { kind: 'running', since: null } }));
    expect(screen.queryByRole('button', { name: 'Play' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Stop' })).toBeEnabled();
  });

  it('play_button_calls_play: When I click Play Then play is requested', async () => {
    const { user, controller } = renderApp(
      createFakeController({ game: steamGame, config: { game_dir: steamGame.root, active_profile: 'vanilla' } }),
    );
    await user.click(screen.getByRole('button', { name: 'Play' }));
    expect(controller.play).toHaveBeenCalledTimes(1);
  });

  it('missing_app_id_error_is_visible: When play fails without App ID Then the alert says so', () => {
    renderApp(createFakeController({ error: { code: 'missing_app_id', message: 'no app id in manifest' } }));
    const alert = screen.getByRole('alert');
    expect(alert).toHaveTextContent('game App ID not found');
    expect(alert).not.toHaveTextContent('no app id in manifest');
  });

  it('error_notice_is_announced: When an error exists Then role alert is shown without a success status', async () => {
    const { user, controller } = renderApp(createFakeController({ error: { code: 'write_failed', message: 'disk' } }));
    expect(screen.getByRole('alert')).toBeInTheDocument();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    await user.click(within(screen.getByRole('alert')).getByRole('button', { name: 'Dismiss' }));
    expect(controller.dismissError).toHaveBeenCalledTimes(1);
  });

  it('success_notice_has_status_role: When a notice exists Then it is announced politely and dismissible', async () => {
    const { user, controller } = renderApp(createFakeController({ notice: { kind: 'imported', count: 2 } }));
    expect(screen.getByRole('status')).toHaveTextContent('2 mods imported');
    await user.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(controller.dismissNotice).toHaveBeenCalledTimes(1);
  });

  it('session_status_uses_cleaned_count: When the session ends Then I see the cleaned count', () => {
    const controller = createFakeController({ session: { kind: 'running', since: null } });
    renderApp(controller);
    expect(screen.getByText('Running…')).toBeInTheDocument();
    act(() => controller.setState({ session: { kind: 'ended', cleaned: 2 } }));
    expect(screen.getByText('Session ended — 2 files removed')).toBeInTheDocument();
  });

  it('never_started_status_is_visible: When the game never started Then I see it', () => {
    renderApp(createFakeController({ session: { kind: 'never-started', cleaned: 3 } }));
    expect(screen.getByText('The game did not open within 2 min; mods removed')).toBeInTheDocument();
  });

  it('default_locale_is_pt_br: When no locale is given Then I see Portuguese', () => {
    render(<App controller={createFakeController()} />);
    expect(screen.getByRole('navigation', { name: 'Navegação principal' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Jogar' })).toBeInTheDocument();
  });

  it('active_profile_selector_calls_set_active_profile: When I pick a profile Then it becomes active', async () => {
    const { user, controller } = renderApp(
      createFakeController({
        profiles: [
          { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
          { id: 'p1', name: 'Teste', mod_ids: [] },
        ],
        config: { game_dir: null, active_profile: 'vanilla' },
      }),
    );
    await user.selectOptions(screen.getByLabelText('Active profile'), 'p1');
    expect(controller.setActiveProfile).toHaveBeenCalledWith('p1');
  });
});
