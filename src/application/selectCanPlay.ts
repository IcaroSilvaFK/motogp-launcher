import type { SelectCanPlay } from './controller';

export const selectCanPlay: SelectCanPlay = (state) =>
  state.status === 'ready' &&
  state.game !== null &&
  state.config.active_profile !== null &&
  !state.busy &&
  state.session.kind !== 'unavailable' &&
  state.session.kind !== 'starting' &&
  state.session.kind !== 'running' &&
  state.session.kind !== 'stopping' &&
  state.session.kind !== 'cleaning-up' &&
  state.staleSession === null;
