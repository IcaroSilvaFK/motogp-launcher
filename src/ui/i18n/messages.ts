import type { ErrorCode } from '../../domain';

export type Locale = 'pt-BR' | 'en';

export interface Messages {
  appName: string;
  loading: string;
  dismiss: string;
  nav: {
    label: string;
    library: string;
    profiles: string;
    settings: string;
  };
  activeProfile: {
    label: string;
  };
  play: {
    button: string;
    starting: string;
  };
  session: {
    running: string;
    ended: (cleaned: number) => string;
    neverStarted: string;
  };
  library: {
    title: string;
    import: string;
    remove: string;
    removeNamed: (name: string) => string;
    size: string;
    fileCount: string;
    files: (count: number) => string;
    empty: string;
    dropHint: string;
  };
  profiles: {
    title: string;
    nameLabel: string;
    renameLabel: string;
    orderTitle: string;
    create: string;
    rename: string;
    renameNamed: (name: string) => string;
    delete: string;
    deleteNamed: (name: string) => string;
    save: string;
    moveUp: (modName: string) => string;
    moveDown: (modName: string) => string;
    mods: string;
    noMods: string;
    vanillaLocked: string;
  };
  settings: {
    title: string;
    game: string;
    noGame: string;
    modsDir: string;
    appId: string;
    appIdNotFound: string;
    detect: string;
    detections: string;
    noDetections: string;
    useThis: string;
    chooseFolder: string;
    language: string;
    languageNames: Record<Locale, string>;
  };
  notices: {
    imported: (count: number) => string;
    modRemoved: string;
    gameConfigured: (name: string) => string;
    profileSaved: string;
    recovered: (cleaned: number) => string;
    gameRunning: string;
  };
  errors: Record<ErrorCode, string>;
}
