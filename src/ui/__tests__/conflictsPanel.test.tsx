// The conflicts panel ignores a preview that resolves after a newer one was requested.
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import type { FileConflict, ModEntry } from '../../domain';
import { App } from '../App';
import { createFakeController } from './fakeController';

const alpha: ModEntry = { id: 'alpha', name: 'Alpha', files: ['Shared_P.pak'], size_bytes: 1 };
const beta: ModEntry = { id: 'beta', name: 'Beta', files: ['Shared_P.pak'], size_bytes: 1 };
const conflict = (mod_ids: string[]): FileConflict => ({
  path: 'Shared_P.pak',
  mod_ids,
  winner: mod_ids.at(-1) ?? '',
});

afterEach(cleanup);

describe('Given the conflicts panel of the profile editor', () => {
  it('When an older preview resolves after a newer one Then the_stale_result_is_ignored', async () => {
    const controller = createFakeController({
      mods: [alpha, beta],
      profiles: [
        { id: 'vanilla', name: 'Vanilla', mod_ids: [] },
        { id: 'mine', name: 'Mine', mod_ids: ['alpha', 'beta'] },
      ],
    });
    const pending: Array<{ ids: string[]; resolve: (value: FileConflict[]) => void }> = [];
    controller.previewConflicts = (ids) =>
      new Promise((resolve) => {
        pending.push({ ids, resolve });
      });
    const user = userEvent.setup();
    render(<App controller={controller} locale="en" />);
    await user.click(
      within(screen.getByRole('navigation', { name: 'Main navigation' })).getByRole('button', { name: 'Profiles' }),
    );
    await user.click(screen.getByRole('button', { name: 'Mine' }));
    await user.click(screen.getByRole('button', { name: 'Move Beta up' }));
    expect(pending.map((call) => call.ids)).toEqual([
      ['alpha', 'beta'],
      ['beta', 'alpha'],
    ]);

    const panel = screen.getByRole('region', { name: 'Conflicts' });
    const [older, newer] = pending;
    await act(async () => newer?.resolve([conflict(['beta', 'alpha'])]));
    await act(async () => older?.resolve([conflict(['alpha', 'beta'])]));

    const mods = within(within(panel).getByRole('list', { name: 'Shared_P.pak' }))
      .getAllByRole('listitem')
      .map((item) => item.textContent);
    expect(mods).toEqual(['Beta', 'Alpha Wins']);
  });
});
