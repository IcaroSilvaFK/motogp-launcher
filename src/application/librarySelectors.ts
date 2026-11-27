import type { FileConflict, ModEntry } from '../domain';
import { modConflictStatus } from '../domain';
import type { LibrarySort } from './ports';

/** Pure selectors of the Library screen: sort order and the "Has conflicts" filter. */

/** A sorted copy: name A-Z (locale aware), or size largest first with the name as tie-break. */
export function sortMods(mods: readonly ModEntry[], sort: LibrarySort): ModEntry[] {
  const byName = (a: ModEntry, b: ModEntry) => a.name.localeCompare(b.name, undefined, { sensitivity: 'base' });
  return [...mods].sort(sort === 'size' ? (a, b) => b.size_bytes - a.size_bytes || byName(a, b) : byName);
}

/** Mods that share at least one file with another mod of the load order (winner or overridden). */
export function selectConflictingMods(mods: readonly ModEntry[], conflicts: readonly FileConflict[]): ModEntry[] {
  return mods.filter((mod) => modConflictStatus([...conflicts], mod.id) !== 'none');
}
