import type { ErrorCode, Locale, UpdateErrorKind } from '../../domain';

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
    unavailable: string;
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
  staleSession: {
    regionLabel: string;
    title: string;
    description: (tracked: number) => string;
    folderLabel: string;
    forget: string;
    clean: string;
    confirmPrompt: string;
    confirm: string;
    cancel: string;
  };
  notices: {
    imported: (count: number) => string;
    modRemoved: string;
    gameConfigured: (name: string) => string;
    profileSaved: string;
    recovered: (cleaned: number) => string;
    gameRunning: string;
    staleForgotten: string;
    staleCleaned: (cleaned: number) => string;
  };
  update: {
    available: (version: string) => string;
    currentVersion: (version: string) => string;
    button: string;
    downloading: string;
    progressLabel: string;
    restarting: string;
    blockedBySession: string;
    errors: Record<UpdateErrorKind, string>;
  };
  errors: Record<ErrorCode, string>;
}
