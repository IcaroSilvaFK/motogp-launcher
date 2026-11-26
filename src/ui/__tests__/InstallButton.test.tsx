import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CatalogInstallState } from '../../domain';
import { I18nProvider } from '../i18n';
import type { Locale } from '../i18n';
import { LauncherContext } from '../LauncherContext';
import { CatalogInstallButton, InstallButton } from '../InstallButton';
import { createFakeController } from './fakeController';

afterEach(cleanup);

function renderButton(state: CatalogInstallState, opts: { disabled?: boolean; locale?: Locale } = {}) {
  const handlers = { onInstall: vi.fn(), onRetry: vi.fn(), onCancel: vi.fn() };
  const view = render(
    <I18nProvider locale={opts.locale ?? 'en'}>
      <InstallButton state={state} disabled={opts.disabled} {...handlers} />
    </I18nProvider>,
  );
  return { ...handlers, ...view };
}

describe('InstallButton', () => {
  it('idle_shows_an_install_button_that_starts_the_install', async () => {
    const { onInstall, container } = renderButton({ status: 'idle' });
    const button = screen.getByRole('button', { name: 'Install' });
    expect(button).not.toHaveAttribute('aria-disabled');
    expect(button).toHaveClass('btn', 'btn-accent-outline', 'btn-sm');
    expect(container.querySelector('.install-control')).toHaveAttribute('data-status', 'idle');
    await userEvent.click(button);
    expect(onInstall).toHaveBeenCalledTimes(1);
  });

  it('idle_disabled_explains_the_session_lock_and_ignores_clicks', async () => {
    const { onInstall } = renderButton({ status: 'idle' }, { disabled: true });
    const button = screen.getByRole('button', { name: 'Install' });
    expect(button).toHaveAttribute('aria-disabled', 'true');
    expect(button).toHaveAccessibleDescription('The library is locked while a game session is running.');
    expect(button).toHaveAttribute('title', 'The library is locked while a game session is running.');
    await userEvent.click(button);
    expect(onInstall).not.toHaveBeenCalled();
  });

  it('downloading_shows_a_determinate_progress_bar_and_cancel', async () => {
    const { onCancel } = renderButton({ status: 'downloading', received: 420, total: 1000, percent: 42 });
    const bar = screen.getByRole('progressbar', { name: 'Download progress' });
    expect(bar).toHaveAttribute('aria-valuenow', '42');
    expect(bar).toHaveAttribute('aria-valuemin', '0');
    expect(bar).toHaveAttribute('aria-valuemax', '100');
    expect(bar).toHaveAttribute('aria-valuetext', 'Downloading 42%');
    expect(screen.getByText('Downloading 42%')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Install' })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('downloading_clamps_the_fill_width', () => {
    const { container } = renderButton({ status: 'downloading', received: 2, total: 1, percent: 140 });
    expect(screen.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '100');
    expect(container.querySelector<HTMLElement>('.install-progress-fill')?.style.width).toBe('100%');
  });

  it.each([
    ['verifying', 'Verifying…'],
    ['importing', 'Installing…'],
  ] as const)('%s_shows_an_indeterminate_progress_bar', (status, label) => {
    const { container } = renderButton({ status });
    const bar = screen.getByRole('progressbar', { name: 'Download progress' });
    expect(bar).not.toHaveAttribute('aria-valuenow');
    expect(bar).toHaveAttribute('aria-valuetext', label);
    expect(bar).toHaveAttribute('data-indeterminate');
    expect(screen.getByText(label)).toBeInTheDocument();
    expect(container.querySelector('.install-control')).toHaveAttribute('aria-busy', 'true');
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('installed_shows_a_confirmed_state_that_is_not_clickable', () => {
    renderButton({ status: 'installed', enabled: false });
    const status = screen.getByRole('status');
    expect(status).toHaveTextContent('Installed');
    expect(status).toHaveClass('badge', 'badge-on');
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('installed_and_enabled_says_so', () => {
    renderButton({ status: 'installed', enabled: true });
    expect(screen.getByRole('status')).toHaveTextContent('Installed and enabled');
  });

  it('failed_shows_the_translated_error_and_retry', async () => {
    const { onRetry } = renderButton({ status: 'failed', code: 'download_hash_mismatch', message: 'sha' });
    const { en } = await import('../i18n/en');
    expect(screen.getByRole('alert')).toHaveTextContent(en.errors.download_hash_mismatch);
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('failed_with_an_unknown_code_falls_back_to_the_failed_label', () => {
    renderButton({ status: 'failed', code: 'not_a_code' as never, message: 'boom' });
    expect(screen.getByRole('alert')).toHaveTextContent('Failed');
  });

  it('retry_is_disabled_during_a_session', async () => {
    const { onRetry } = renderButton(
      { status: 'failed', code: 'download_hash_mismatch', message: 'x' },
      { disabled: true },
    );
    const retry = screen.getByRole('button', { name: 'Retry' });
    expect(retry).toHaveAttribute('aria-disabled', 'true');
    await userEvent.click(retry);
    expect(onRetry).not.toHaveBeenCalled();
  });

  it('renders_in_portuguese', () => {
    renderButton({ status: 'downloading', received: 1, total: 2, percent: 50 }, { locale: 'pt-BR' });
    expect(screen.getByRole('button', { name: 'Cancelar' })).toBeInTheDocument();
  });
});

describe('CatalogInstallButton', () => {
  it('reads_the_entry_state_and_drives_the_controller', async () => {
    const controller = createFakeController();
    const { rerender } = render(
      <I18nProvider locale="en">
        <LauncherContext.Provider value={{ controller, state: controller.getState() }}>
          <CatalogInstallButton entryId="rebalanced-ai" />
        </LauncherContext.Provider>
      </I18nProvider>,
    );
    await userEvent.click(screen.getByRole('button', { name: 'Install' }));
    expect(controller.installCatalogEntry).toHaveBeenCalledWith('rebalanced-ai', { enable: true });

    act(() =>
      controller.setState({
        catalogInstalls: { 'rebalanced-ai': { status: 'failed', code: 'download_hash_mismatch', message: 'x' } },
      }),
    );
    rerender(
      <I18nProvider locale="en">
        <LauncherContext.Provider value={{ controller, state: controller.getState() }}>
          <CatalogInstallButton entryId="rebalanced-ai" />
        </LauncherContext.Provider>
      </I18nProvider>,
    );
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(controller.installCatalogEntry).toHaveBeenCalledTimes(2);

    act(() =>
      controller.setState({
        catalogInstalls: { 'rebalanced-ai': { status: 'downloading', received: 1, total: 4, percent: 25 } },
      }),
    );
    rerender(
      <I18nProvider locale="en">
        <LauncherContext.Provider value={{ controller, state: controller.getState() }}>
          <CatalogInstallButton entryId="rebalanced-ai" />
        </LauncherContext.Provider>
      </I18nProvider>,
    );
    await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(controller.cancelCatalogInstall).toHaveBeenCalledWith('rebalanced-ai');
  });

  it('is_disabled_while_a_session_runs', () => {
    const controller = createFakeController({ session: { kind: 'running', since: 1 } });
    render(
      <I18nProvider locale="en">
        <LauncherContext.Provider value={{ controller, state: controller.getState() }}>
          <CatalogInstallButton entryId="rebalanced-ai" />
        </LauncherContext.Provider>
      </I18nProvider>,
    );
    expect(screen.getByRole('button', { name: 'Install' })).toHaveAttribute('aria-disabled', 'true');
  });
});
