import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import type { AdPreferenceStore } from '../../application';
import { App } from '../App';
import { createFakeController } from './fakeController';

function store(): AdPreferenceStore & { value: 'on' | 'off' | null } {
  const s = {
    value: null as 'on' | 'off' | null,
    load: () => s.value,
    save: (v: 'on' | 'off') => {
      s.value = v;
    },
  };
  return s;
}

const bar = () => screen.queryByRole('region', { name: 'Sponsored content' });

afterEach(cleanup);

describe('AdConsentBar', () => {
  it('is hidden while a game session is running', () => {
    const controller = createFakeController({ session: { kind: 'running' } as never });
    render(<App controller={controller} locale="en" adPreferenceStore={store()} />);
    expect(bar()).not.toBeInTheDocument();
  });

  it('is hidden while the launcher loads', () => {
    const controller = createFakeController({ status: 'loading' });
    render(<App controller={controller} locale="en" adPreferenceStore={store()} />);
    expect(bar()).not.toBeInTheDocument();
  });

  it('is hidden without a store', () => {
    render(<App controller={createFakeController()} locale="en" />);
    expect(bar()).not.toBeInTheDocument();
  });

  it('saves "on" when dismissed', async () => {
    const s = store();
    render(<App controller={createFakeController()} locale="en" adPreferenceStore={s} />);
    await userEvent.setup().click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(s.value).toBe('on');
    expect(bar()).not.toBeInTheDocument();
  });

  it('shows the Portuguese copy', () => {
    render(<App controller={createFakeController()} locale="pt-BR" adPreferenceStore={store()} />);
    expect(screen.getByRole('button', { name: 'Desativar anúncios' })).toBeInTheDocument();
  });
});
