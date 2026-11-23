import { ERROR_CODES, VANILLA_ID } from './types';
import type { CommandError, ErrorCode, FileConflict, Profile } from './types';

export const SUPPORTED_MOD_EXTENSIONS = ['pak', 'utoc', 'ucas', 'zip'] as const;

function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === 'string' && (ERROR_CODES as readonly string[]).includes(value);
}

/** Normalizes anything a rejected command may throw into a CommandError. */
export function toCommandError(error: unknown): CommandError {
  if (typeof error === 'object' && error !== null && 'code' in error) {
    const { code, message } = error as { code: unknown; message?: unknown };
    return {
      code: isErrorCode(code) ? code : 'unknown',
      message: typeof message === 'string' ? message : '',
    };
  }
  if (error instanceof Error) return { code: 'unknown', message: error.message };
  return { code: 'unknown', message: String(error) };
}

export function isSupportedModFile(path: string): boolean {
  const name = path.split(/[\\/]/).pop() ?? '';
  const dot = name.lastIndexOf('.');
  if (dot <= 0) return false;
  const extension = name.slice(dot + 1).toLowerCase();
  return (SUPPORTED_MOD_EXTENSIONS as readonly string[]).includes(extension);
}

/** Returns a new list with the item at `index` moved by `delta`; out of range returns a copy. */
export function moveItem<T>(list: readonly T[], index: number, delta: number): T[] {
  const target = index + delta;
  const copy = [...list];
  if (index < 0 || index >= copy.length || target < 0 || target >= copy.length) return copy;
  const [item] = copy.splice(index, 1);
  copy.splice(target, 0, item as T);
  return copy;
}

export function toggleMod(modIds: readonly string[], id: string): string[] {
  return modIds.includes(id) ? modIds.filter((current) => current !== id) : [...modIds, id];
}

export function isVanilla(profile: Profile): boolean {
  return profile.id === VANILLA_ID;
}

export type ModConflictStatus = 'none' | 'wins' | 'overridden';

/** Conflicts that involve the mod. */
export function conflictsOfMod(conflicts: FileConflict[], modId: string): FileConflict[] {
  return conflicts.filter((conflict) => conflict.mod_ids.includes(modId));
}

/** `overridden` if another mod wins any of its files, `wins` if it only wins, `none` without conflicts. */
export function modConflictStatus(conflicts: FileConflict[], modId: string): ModConflictStatus {
  const involved = conflictsOfMod(conflicts, modId);
  if (involved.length === 0) return 'none';
  return involved.some((conflict) => conflict.winner !== modId) ? 'overridden' : 'wins';
}
