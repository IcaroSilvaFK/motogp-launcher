import { createContext, useContext, useEffect } from 'react';
import type { RefObject } from 'react';
import type { ScreenId } from './Sidebar';

/**
 * What the current screen exposes to the global shortcuts (#157): its search field and a "go back"
 * action. Screens register through the hooks below; nothing queries the DOM for them.
 */
export class ShortcutRegistry {
  private searchRef: RefObject<HTMLInputElement | null> | null = null;
  private backAction: (() => void) | null = null;

  /** The registered search input, if any screen is mounted with one. */
  searchInput(): HTMLInputElement | null {
    return this.searchRef?.current ?? null;
  }

  /** Runs the registered back action; false when none is registered. */
  goBack(): boolean {
    if (this.backAction === null) return false;
    this.backAction();
    return true;
  }

  /** Registers the search ref; the returned function unregisters it. */
  setSearch(ref: RefObject<HTMLInputElement | null>): () => void {
    this.searchRef = ref;
    return () => {
      if (this.searchRef === ref) this.searchRef = null;
    };
  }

  /** Registers the back action; the returned function unregisters it. */
  setBack(action: () => void): () => void {
    this.backAction = action;
    return () => {
      if (this.backAction === action) this.backAction = null;
    };
  }
}

export function createShortcutRegistry(): ShortcutRegistry {
  return new ShortcutRegistry();
}

export const ShortcutContext = createContext<ShortcutRegistry>(createShortcutRegistry());

/** Makes the ref's input the target of "/" and Ctrl+F while the calling screen is mounted. */
export function useShortcutSearch(ref: RefObject<HTMLInputElement | null>): void {
  const registry = useContext(ShortcutContext);
  useEffect(() => registry.setSearch(ref), [registry, ref]);
}

/** Makes onBack the action of Esc while the caller is mounted; null registers nothing. */
export function useShortcutBack(onBack: (() => void) | null): void {
  const registry = useContext(ShortcutContext);
  useEffect(() => (onBack === null ? undefined : registry.setBack(onBack)), [registry, onBack]);
}

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
  return (
    target instanceof HTMLInputElement && !['button', 'checkbox', 'radio', 'submit', 'reset'].includes(target.type)
  );
}

/** The welcome and confirm dialogs, and any other modal, handle the keyboard themselves. */
function dialogOpen(): boolean {
  return document.querySelector('[role="dialog"], [role="alertdialog"], [aria-modal="true"]') !== null;
}

/**
 * The single keyboard-shortcut listener (#157): Esc goes back, "/" and Ctrl+F focus the search,
 * Ctrl+1..4 switch sections in sidebar order. Events another handler already consumed
 * (`defaultPrevented`) and shortcuts pressed inside a dialog are ignored.
 */
export function useShortcuts(
  registry: ShortcutRegistry,
  screens: readonly ScreenId[],
  onSelect: (screen: ScreenId) => void,
) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.isComposing || dialogOpen()) return;
      const typing = isEditable(event.target);

      if (event.key === 'Escape') {
        if (typing) return;
        if (registry.goBack()) event.preventDefault();
        return;
      }

      const input = registry.searchInput();
      const ctrl = (event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey;

      if (ctrl && event.key.toLowerCase() === 'f') {
        if (input === null) return;
        event.preventDefault();
        input.focus();
        input.select();
        return;
      }

      if (event.key === '/' && !event.ctrlKey && !event.metaKey && !event.altKey) {
        if (typing || input === null) return;
        event.preventDefault();
        input.focus();
        return;
      }

      if (ctrl && /^[1-9]$/.test(event.key)) {
        const screen = screens[Number(event.key) - 1];
        if (screen === undefined) return;
        event.preventDefault();
        onSelect(screen);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [registry, screens, onSelect]);
}
