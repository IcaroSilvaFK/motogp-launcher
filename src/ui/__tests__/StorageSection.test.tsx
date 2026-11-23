// Edge cases of the Storage section not covered by storage.feature: stale answers, inline
// failures after a successful measurement, the open-folder failure and the meter colors.
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { readFileSync } from 'node:fs';
import { StrictMode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { StorageUsage } from '../../domain';
import { I18nProvider } from '../i18n';
import { LauncherContext } from '../LauncherContext';
import { StorageSection } from '../StorageSection';
import { createFakeController } from './fakeController';
import type { FakeController } from './fakeController';

const GB = 1024 ** 3;

function usage(totalGb: number): StorageUsage {
  return {
    // An accented Windows profile folder on purpose: real users have them.
    root: 'C:\\Users\\Usuário Padrão\\AppData\\Roaming\\motogp-launcher',
    totalBytes: totalGb * GB,
    freeBytes: 50 * GB,
    categories: [
      { category: 'mods', bytes: totalGb * GB },
      { category: 'launcher_data', bytes: 0 },
      { category: 'other', bytes: 0 },
    ],
  };
}

function pending<T>() {
  let resolve: (value: T) => void = () => undefined;
  let reject: (reason: unknown) => void = () => undefined;
  const promise = new Promise<T>((ok, ko) => {
    resolve = ok;
    reject = ko;
  });
  return { promise, resolve, reject };
}

function mount(controller: FakeController) {
  return render(
    <I18nProvider locale="en">
      <LauncherContext.Provider value={{ controller, state: controller.getState() }}>
        <StorageSection />
      </LauncherContext.Provider>
    </I18nProvider>,
  );
}

const region = () => screen.getByRole('region', { name: 'Storage' });
const total = () => within(region()).getByText('Total used', { selector: 'dt' }).nextElementSibling as HTMLElement;

describe('Given the Storage section', () => {
  afterEach(cleanup);

  it('When the first measurement is running Then skeletons are shown and the card is busy', async () => {
    const first = pending<StorageUsage>();
    const controller = createFakeController();
    controller.getStorageUsage = vi.fn(() => first.promise);
    mount(controller);
    expect(region().querySelector('.storage-card')).toHaveAttribute('aria-busy', 'true');
    expect(region().querySelectorAll('.skeleton').length).toBeGreaterThan(0);
    expect(within(region()).queryAllByRole('meter')).toHaveLength(0);
    expect(within(region()).getByRole('status')).toHaveTextContent('Measuring…');
    await act(async () => first.resolve(usage(1)));
    expect(region().querySelector('.skeleton')).toBeNull();
    expect(within(region()).getByRole('status')).toHaveTextContent('');
  });

  it('When an older answer arrives after a newer one Then the older answer is ignored', async () => {
    // StrictMode mounts the effect twice, so two measurements are in flight at once.
    const older = pending<StorageUsage>();
    const newer = pending<StorageUsage>();
    const answers = [older, newer];
    const controller = createFakeController();
    controller.getStorageUsage = vi.fn(() => (answers.shift() ?? newer).promise);
    render(
      <StrictMode>
        <I18nProvider locale="en">
          <LauncherContext.Provider value={{ controller, state: controller.getState() }}>
            <StorageSection />
          </LauncherContext.Provider>
        </I18nProvider>
      </StrictMode>,
    );
    expect(controller.getStorageUsage).toHaveBeenCalledTimes(2);
    await act(async () => newer.resolve(usage(3)));
    expect(total()).toHaveTextContent('3 GB');
    await act(async () => older.resolve(usage(1)));
    expect(total()).toHaveTextContent('3 GB');
    expect(region().querySelector('.storage-card')).toHaveAttribute('aria-busy', 'false');
  });

  it('When Retry succeeds after a failure Then the error is cleared and the values are shown', async () => {
    const controller = createFakeController();
    controller.getStorageUsage = vi
      .fn()
      .mockRejectedValueOnce({ code: 'read_failed', message: 'locked' })
      .mockResolvedValueOnce(usage(4));
    const user = userEvent.setup();
    mount(controller);
    const alert = await within(region()).findByRole('alert');
    await user.click(within(alert).getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(total()).toHaveTextContent('4 GB'));
    expect(within(region()).queryByRole('alert')).toBeNull();
  });

  it('When a refresh fails after a successful measurement Then the previous values stay with the inline error', async () => {
    const controller = createFakeController();
    let calls = 0;
    controller.getStorageUsage = vi.fn(() => {
      calls += 1;
      return calls === 1 ? Promise.resolve(usage(2)) : Promise.reject(new Error('disk gone'));
    });
    const user = userEvent.setup();
    mount(controller);
    await waitFor(() => expect(total()).toHaveTextContent('2 GB'));
    await user.click(within(region()).getByRole('button', { name: 'Refresh' }));
    const alert = await within(region()).findByRole('alert');
    expect(alert).toHaveTextContent('Could not measure the data folder.');
    expect(alert).toHaveTextContent('Something went wrong');
    expect(total()).toHaveTextContent('2 GB');
    expect(within(region()).getAllByRole('meter')).toHaveLength(3);
  });

  it('When the first measurement fails Then no value is guessed and the folder hint is shown', async () => {
    const controller = createFakeController();
    controller.getStorageUsage = vi.fn(() => Promise.reject({ code: 'task_failed', message: 'x' }));
    mount(controller);
    await within(region()).findByRole('alert');
    expect(total()).toHaveTextContent('—');
    expect(within(region()).queryAllByRole('meter')).toHaveLength(0);
    expect(within(region()).getByText(/are kept here/)).toBeInTheDocument();
    expect(within(region()).getByRole('alert')).toHaveTextContent('A background task failed.');
  });

  it('When the answer arrives after the section is gone Then nothing is updated', async () => {
    const first = pending<StorageUsage>();
    const controller = createFakeController();
    controller.getStorageUsage = vi.fn(() => first.promise);
    const view = mount(controller);
    view.unmount();
    await act(async () => first.resolve(usage(1)));
    expect(screen.queryByRole('region', { name: 'Storage' })).toBeNull();
  });

  it('When opening the folder fails Then an inline message is shown and cleared on the next try', async () => {
    const controller = createFakeController();
    controller.getStorageUsage = vi.fn(() => Promise.resolve(usage(1)));
    controller.openDataFolder = vi
      .fn()
      .mockRejectedValueOnce({ code: 'task_failed', message: 'no shell' })
      .mockResolvedValueOnce(undefined);
    const user = userEvent.setup();
    mount(controller);
    await waitFor(() => expect(total()).toHaveTextContent('1 GB'));

    await user.click(within(region()).getByRole('button', { name: 'Open folder' }));
    expect(await within(region()).findByRole('alert')).toHaveTextContent('Could not open the data folder.');

    await user.click(within(region()).getByRole('button', { name: 'Open folder' }));
    await waitFor(() => expect(within(region()).queryByRole('alert')).toBeNull());
    expect(controller.openDataFolder).toHaveBeenCalledTimes(2);
  });

  it('Then the meter fill and track are neutral tokens with a non-text contrast of at least 3', () => {
    const css = readFileSync(`${import.meta.dirname}/../styles.css`, 'utf8');
    const root = /:root\s*\{([^}]*)\}/.exec(css)?.[1] ?? '';
    const read = (name: string) => new RegExp(`${name}\\s*:\\s*(#[0-9a-fA-F]{6})`).exec(root)?.[1] ?? '';
    const lum = (hex: string) => {
      const [r = 0, g = 0, b = 0] = [1, 3, 5].map((i) => {
        const c = parseInt(hex.slice(i, i + 2), 16) / 255;
        return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const fill = read('--meter-fill');
    const track = read('--meter-track');
    expect(fill).not.toBe('');
    expect(track).not.toBe('');
    expect(fill.toLowerCase()).not.toBe(read('--accent').toLowerCase());
    const [hi = 0, lo = 0] = [lum(fill), lum(track)].sort((x, y) => y - x);
    expect((hi + 0.05) / (lo + 0.05)).toBeGreaterThanOrEqual(3);
    expect(css).toMatch(/\.storage-meter-fill\s*\{[^}]*background:\s*var\(--meter-fill\)/);
  });
});
