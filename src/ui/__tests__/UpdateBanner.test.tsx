import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { UpdateController, UpdateState } from '../../application';
import type { UpdateInfo } from '../../domain';
import { App } from '../App';
import { I18nProvider } from '../i18n';
import type { Locale } from '../i18n';
import { UpdateBanner } from '../UpdateBanner';
import { createFakeController } from './fakeController';

afterEach(cleanup);

const info: UpdateInfo = { version: '1.2.0', currentVersion: '1.1.0', notes: 'Fixes\n<b>bold</b>', date: null };

const renderBanner = (state: UpdateState, opts: { installBlocked?: boolean; locale?: Locale } = {}) => {
  const onUpdate = vi.fn();
  render(
    <I18nProvider locale={opts.locale ?? 'en'}>
      <UpdateBanner state={state} installBlocked={opts.installBlocked ?? false} onUpdate={onUpdate} />
    </I18nProvider>,
  );
  return { onUpdate };
};

describe('UpdateBanner', () => {
  it('shows_new_version_notes_and_update_button', async () => {
    const { onUpdate } = renderBanner({ kind: 'available', info });
    expect(screen.getByRole('heading', { name: 'New version 1.2.0 available' })).toBeInTheDocument();
    expect(screen.getByText(/Fixes/)).toBeInTheDocument();
    expect(screen.getByText(/<b>bold<\/b>/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Update' }));
    expect(onUpdate).toHaveBeenCalledTimes(1);
    cleanup();
    renderBanner({ kind: 'available', info }, { locale: 'pt-BR' });
    expect(screen.getByRole('heading', { name: 'Nova versão 1.2.0 disponível' })).toBeInTheDocument();
  });

  it('hides_banner_when_up_to_date', () => {
    const { container } = render(
      <I18nProvider locale="en">
        <UpdateBanner state={{ kind: 'none' }} installBlocked={false} onUpdate={vi.fn()} />
        <UpdateBanner state={{ kind: 'idle' }} installBlocked={false} onUpdate={vi.fn()} />
      </I18nProvider>,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('shows_determinate_or_indeterminate_progress', () => {
    renderBanner({ kind: 'downloading', info, downloaded: 50, total: 200 });
    const bar = screen.getByRole('progressbar');
    expect(bar).toHaveAttribute('max', '200');
    expect(bar).toHaveAttribute('value', '50');
    expect(screen.getByRole('button', { name: 'Update' })).toBeDisabled();
    cleanup();
    renderBanner({ kind: 'downloading', info, downloaded: 50, total: null });
    expect(screen.getByRole('progressbar')).not.toHaveAttribute('value');
    expect(screen.getByRole('progressbar', { name: 'Downloading update' })).toBeInTheDocument();
  });

  it('shows_restarting_when_installed', () => {
    renderBanner({ kind: 'installed', info });
    expect(screen.getByText('Update installed. Restarting…')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Update' })).toBeDisabled();
  });

  it('update_error_is_accessible', async () => {
    const { onUpdate } = renderBanner({ kind: 'error', info, error: { kind: 'network', message: 'offline' } });
    const alert = screen.getByRole('alert');
    expect(alert).not.toHaveTextContent('offline');
    expect(alert).toHaveTextContent('Could not reach the update server');
    expect(screen.getByText(/1\.2\.0/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Update' }));
    expect(onUpdate).toHaveBeenCalledTimes(1);
    cleanup();
    for (const kind of ['signature', 'install', 'session', 'busy', 'unknown'] as const) {
      renderBanner({ kind: 'error', info: null, error: { kind, message: `m-${kind}` } });
      expect(screen.getByRole('alert')).not.toHaveTextContent(`m-${kind}`);
      expect(screen.queryByRole('button', { name: 'Update' })).not.toBeInTheDocument();
      cleanup();
    }
  });

  it('session_state_disables_update_action', () => {
    renderBanner({ kind: 'available', info }, { installBlocked: true });
    expect(screen.getByRole('button', { name: 'Update' })).toBeDisabled();
    expect(screen.getByText(/session/i)).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /1\.2\.0/ })).toBeInTheDocument();
  });
});

describe('App with an updater', () => {
  it('renders_banner_and_installs_on_click', async () => {
    let state: UpdateState = { kind: 'available', info };
    const install = vi.fn(() => Promise.resolve());
    const updater: UpdateController = {
      getState: () => state,
      subscribe: () => () => undefined,
      check: vi.fn(() => Promise.resolve()),
      install,
    };
    render(<App controller={createFakeController()} locale="en" updater={updater} />);
    await userEvent.click(screen.getByRole('button', { name: 'Update' }));
    expect(install).toHaveBeenCalledTimes(1);
    expect(updater.check).not.toHaveBeenCalled();
    state = { kind: 'none' };
  });

  it('blocks_install_while_session_runs', () => {
    const available: UpdateState = { kind: 'available', info };
    const updater: UpdateController = {
      getState: () => available,
      subscribe: () => () => undefined,
      check: vi.fn(() => Promise.resolve()),
      install: vi.fn(() => Promise.resolve()),
    };
    render(
      <App
        controller={createFakeController({ session: { kind: 'running', since: null } })}
        locale="en"
        updater={updater}
      />,
    );
    expect(screen.getByRole('button', { name: 'Update' })).toBeDisabled();
  });
});
