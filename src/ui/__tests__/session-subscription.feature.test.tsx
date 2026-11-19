// End-to-end scenario of tests/features/frontend/session-subscription.feature: the real
// application controller and the real UI, with only the ports replaced by the in-memory fakes.
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { createLauncherController } from '../../application';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from '../../application/__tests__/fakes';
import type { GameInfo } from '../../domain';
import { App } from '../App';

afterEach(cleanup);

const game: GameInfo = {
  name: 'MotoGP 25',
  root: 'D:/Steam/MotoGP25',
  mods_dir: 'D:/Steam/MotoGP25/MotoGP25/Content/Paks/~mods',
  app_id: 1234,
};

describe('Given the session event subscription rejects', () => {
  it('When the launcher starts Then an error notice is announced and Play stays disabled', async () => {
    const controller = createLauncherController({
      gateway: new FakeGateway({ game }),
      picker: new FakePicker(),
      session: new FakeSessionEvents({ rejectSession: new Error('listen failed') }),
      drop: new FakeDropEvents(),
    });
    render(<App controller={controller} locale="en" />);
    await waitFor(() => expect(controller.getState().status).toBe('ready'));

    expect(await screen.findByRole('alert')).toHaveTextContent('Session events are unavailable');
    expect(screen.getByRole('button', { name: 'Play' })).toBeDisabled();
    expect(screen.getByRole('status')).toHaveTextContent('Play is disabled');
  });
});
