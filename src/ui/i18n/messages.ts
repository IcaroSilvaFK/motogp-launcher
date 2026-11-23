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
    name: string;
    search: string;
    searchPlaceholder: string;
    noResults: string;
    inActiveProfile: string;
    notInActiveProfile: string;
    summary: (total: number, inProfile: number, profileName: string | null) => string;
    filterLabel: string;
    filterAll: string;
    clearFilters: string;
    /** Accessible name of the row switch; `profileName` is null without an active profile. */
    toggleNamed: (modName: string, profileName: string | null) => string;
    toggleLocked: string;
    toggleVanilla: string;
    toggleNoProfile: string;
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
    createTitle: string;
    createHint: string;
    listTitle: string;
    summary: (count: number, activeName: string | null) => string;
    modCount: (count: number) => string;
    active: string;
    selectHint: string;
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
    gameHint: string;
    preferences: string;
    languageHint: string;
    language: string;
    languageNames: Record<Locale, string>;
    subtitle: string;
    sectionsLabel: string;
    folderTitle: string;
  };
  statusBar: {
    steam: string;
    noGame: string;
    setUp: string;
    ready: string;
    version: (version: string) => string;
    versionTitle: string;
  };
  about: {
    title: string;
    versionLabel: string;
    versionHint: string;
    versionUnknown: string;
    updatesLabel: string;
    updatesHint: string;
    check: string;
    checking: string;
    upToDate: string;
    available: (version: string) => string;
    install: string;
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
