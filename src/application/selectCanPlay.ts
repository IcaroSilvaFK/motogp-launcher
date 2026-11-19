import type { SelectCanPlay } from './controller';

export const selectCanPlay: SelectCanPlay = (state) =>
  state.status === 'ready' &&
  state.game !== null &&
  state.config.active_profile !== null &&
  !state.busy &&
  state.session.kind !== 'starting' &&
  state.session.kind !== 'running';
