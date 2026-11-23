// Screen-level tests of the stale session recovery notice, against a fake controller.
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import type { Locale } from '../i18n';
import { App } from '../App';
import { createFakeController } from './fakeController';

afterEach(cleanup);

const OLD = 'D:/old/Content/Paks/~mods';
const stale = { modsDir: OLD, tracked: 3 };

const renderApp = (patch = {}, locale: Locale = 'en') => {
  const controller = createFakeController({ staleSession: stale, ...patch });
  const user = userEvent.setup();
  render(<App controller={controller} locale={locale} />);
  return { user, controller };
};

const region = () => screen.getByRole('region', { name: 'Old session pending' });

describe('Given a stale session', () => {
  it('stale_notice_names_the_folder_and_the_file_count', () => {
    renderApp();
    expect(within(region()).getByText(OLD)).toBeInTheDocument();
    expect(within(region()).getByText(/3 files/)).toBeInTheDocument();
    expect(within(region()).getByRole('button', { name: 'Forget the old session' })).toBeEnabled();
    expect(within(region()).getByRole('button', { name: 'Clean the old folder' })).toBeEnabled();
  });

  it('stale_notice_is_hidden_without_a_stale_session', () => {
    renderApp({ staleSession: null });
    expect(screen.queryByRole('region', { name: 'Old session pending' })).not.toBeInTheDocument();
  });

  it('forget_button_calls_the_controller', async () => {
    const { user, controller } = renderApp();
    await user.click(screen.getByRole('button', { name: 'Forget the old session' }));
    expect(controller.forgetStaleSession).toHaveBeenCalledTimes(1);
    expect(controller.cleanStaleSession).not.toHaveBeenCalled();
  });

  it('clean_asks_for_confirmation_naming_the_folder_before_calling', async () => {
    const { user, controller } = renderApp();
    await user.click(screen.getByRole('button', { name: 'Clean the old folder' }));

    expect(controller.cleanStaleSession).not.toHaveBeenCalled();
    expect(within(region()).getByText(OLD)).toBeInTheDocument();
    expect(within(region()).getByText(/only files the launcher deployed/i)).toBeInTheDocument();
    expect(within(region()).getByRole('button', { name: 'Confirm' })).toBeEnabled();
    expect(within(region()).queryByRole('button', { name: 'Forget the old session' })).not.toBeInTheDocument();
  });

  it('confirming_calls_clean', async () => {
    const { user, controller } = renderApp();
    await user.click(screen.getByRole('button', { name: 'Clean the old folder' }));
    await user.click(screen.getByRole('button', { name: 'Confirm' }));
    expect(controller.cleanStaleSession).toHaveBeenCalledTimes(1);
  });

  it('cancelling_returns_to_the_notice_without_calling_anything', async () => {
    const { user, controller } = renderApp();
    await user.click(screen.getByRole('button', { name: 'Clean the old folder' }));
    await user.click(screen.getByRole('button', { name: 'Cancel' }));

    expect(controller.cleanStaleSession).not.toHaveBeenCalled();
    expect(controller.forgetStaleSession).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Clean the old folder' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Confirm' })).not.toBeInTheDocument();
  });

  it('buttons_are_disabled_while_busy', async () => {
    const { user, controller } = renderApp({ busy: true });
    expect(screen.getByRole('button', { name: 'Forget the old session' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Clean the old folder' })).toBeDisabled();

    act(() => controller.setState({ busy: false }));
    await user.click(await screen.findByRole('button', { name: 'Clean the old folder' }));
    act(() => controller.setState({ busy: true }));
    expect(await screen.findByRole('button', { name: 'Confirm' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled();
  });

  it('the_confirmation_closes_when_the_stale_session_goes_away', async () => {
    const { user, controller } = renderApp();
    await user.click(screen.getByRole('button', { name: 'Clean the old folder' }));
    act(() => controller.setState({ staleSession: null }));
    expect(screen.queryByRole('region', { name: 'Old session pending' })).not.toBeInTheDocument();
    act(() => controller.setState({ staleSession: stale }));
    expect(await screen.findByRole('button', { name: 'Clean the old folder' })).toBeInTheDocument();
  });

  // The Portuguese literals below are intentional: this test proves the visible pt-BR copy.
  it('shows_the_folder_in_pt_BR_too', async () => {
    const { user } = renderApp({}, 'pt-BR');
    const area = screen.getByRole('region', { name: 'Sessão antiga pendente' });
    expect(within(area).getByText(OLD)).toBeInTheDocument();
    expect(within(area).getByText(/3 arquivos/)).toBeInTheDocument();
    await user.click(within(area).getByRole('button', { name: 'Limpar a pasta antiga' }));
    expect(within(area).getByText(OLD)).toBeInTheDocument();
    expect(within(area).getByRole('button', { name: 'Confirmar' })).toBeInTheDocument();
    expect(within(area).getByRole('button', { name: 'Cancelar' })).toBeInTheDocument();
  });

  it('uses_the_singular_for_one_file', () => {
    renderApp({ staleSession: { modsDir: OLD, tracked: 1 } });
    expect(within(region()).getByText(/1 file\b/)).toBeInTheDocument();
  });

  it('stale_notices_have_texts', () => {
    renderApp({ staleSession: null, notice: { kind: 'stale-forgotten' } });
    expect(screen.getByRole('status')).toHaveTextContent('Old session forgotten');
    cleanup();
    renderApp({ staleSession: null, notice: { kind: 'stale-cleaned', cleaned: 3 } });
    expect(screen.getByRole('status')).toHaveTextContent('Old folder cleaned; 3 files removed');
  });
});
