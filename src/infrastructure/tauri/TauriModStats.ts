import { invoke } from '@tauri-apps/api/core';
import type { ModStatsSource } from '../../application';
import { ModStatsError, parseModStats, toModStatsError } from '../../domain';
import type { ModStats } from '../../domain';

async function change(command: 'community_star' | 'community_unstar', id: string): Promise<ModStats> {
  let answer: unknown;
  try {
    answer = await invoke(command, { id });
  } catch (error) {
    throw toModStatsError(error);
  }
  const stats = parseModStats(answer);
  if (!stats) throw new ModStatsError('failed');
  return stats;
}

/**
 * Mod stars over IPC (`community_mod_stats`, `community_star`, `community_unstar`). `stats` never
 * rejects; `star` and `unstar` reject with a typed `ModStatsError`.
 */
export function createTauriModStats(): ModStatsSource {
  return {
    async stats() {
      try {
        const answer = (await invoke('community_mod_stats')) as { mods?: unknown } | null;
        if (!Array.isArray(answer?.mods)) return [];
        return answer.mods.flatMap((item) => parseModStats(item) ?? []);
      } catch (error) {
        // Expected while the community server is down or the backend lacks the command.
        console.warn('community_mod_stats failed', error);
        return [];
      }
    },
    star: (id) => change('community_star', id),
    unstar: (id) => change('community_unstar', id),
  };
}
