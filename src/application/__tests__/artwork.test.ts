import { describe, expect, it } from 'vitest';
import type { GameInfo } from '../../domain';
import { createLauncherController } from '../createLauncherController';
import { FakeDropEvents, FakeGateway, FakePicker, FakeSessionEvents } from './fakes';
import type { FakeGatewayOptions } from './fakes';

const game: GameInfo = { name: 'MotoGP25', root: 'C:/g', mods_dir: 'C:/g/~mods', app_id: 1234 };
const ART = 'data:image/jpeg;base64,AQID';

async function ready(options: FakeGatewayOptions = {}) {
  const gateway = new FakeGateway({ game, ...options });
  const picker = new FakePicker();
  const controller = createLauncherController({
    gateway,
    picker,
    session: new FakeSessionEvents(),
    drop: new FakeDropEvents(),
  });
  await controller.start();
  return { gateway, picker, controller };
}

describe('Given the game artwork in the launcher state', () => {
  it('When started with a game that has artwork Then the state carries the data URL', async () => {
    const { controller } = await ready({ artwork: ART });
    expect(controller.getState().artwork).toBe(ART);
  });

  it('When the game has no cached artwork Then the artwork is null', async () => {
    const { controller } = await ready({ artwork: null });
    expect(controller.getState().artwork).toBeNull();
  });

  it('When no game is configured Then the artwork is null and the backend is not asked', async () => {
    const { controller, gateway } = await ready({ game: null, artwork: ART });
    expect(controller.getState().artwork).toBeNull();
    expect(gateway.callsOf('getGameArtwork')).toHaveLength(0);
  });

  it('When fetching the artwork fails Then the artwork is null and no error is raised', async () => {
    const gateway = new FakeGateway({ game, artwork: ART });
    gateway.failNext('getGameArtwork', { code: 'read_failed', message: 'boom' });
    const controller = createLauncherController({
      gateway,
      picker: new FakePicker(),
      session: new FakeSessionEvents(),
      drop: new FakeDropEvents(),
    });
    await controller.start();
    expect(controller.getState().artwork).toBeNull();
    expect(controller.getState().error).toBeNull();
  });

  it('When a game is chosen Then the artwork is fetched for it', async () => {
    const { controller, picker } = await ready({ game: null, artwork: ART });
    expect(controller.getState().artwork).toBeNull();
    picker.queueDirectory('C:/Jogos/MotoGP');
    await controller.chooseGameDirectory();
    expect(controller.getState().artwork).toBe(ART);
  });
});
