import type { ErrorCode, Locale, StorageCategory, UpdateErrorKind } from '../../domain';

export interface Messages {
  appName: string;
  loading: string;
  dismiss: string;
  ads: {
    consentLabel: string;
    consentText: string;
    keep: string;
    turnOff: string;
    /** Visible on every sponsored card and row; also the slot's accessible name (#104). */
    sponsored: string;
    learnMore: string;
    /** Accessible description of the action: the page opens outside the launcher. */
    opensInBrowser: string;
  };
  nav: {
    label: string;
    library: string;
    profiles: string;
    settings: string;
    discover: string;
    collapse: string;
    expand: string;
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
    runningSince: (time: string) => string;
    stopping: string;
    cleaningUp: string;
    ended: (cleaned: number) => string;
    neverStarted: string;
    unavailable: string;
  };
  stop: {
    button: string;
    confirmTitle: string;
    confirmDetail: string;
    confirm: string;
    cancel: string;
    stopping: string;
    cleaningUp: string;
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
    /** Reason the Remove buttons are inert while a game session is running. */
    removeLocked: string;
    toggleVanilla: string;
    toggleNoProfile: string;
    /** Conflict badge of a row whose mod wins every file it shares in the active profile. */
    conflictWins: string;
    /** Conflict badge of a row whose mod loses at least one shared file in the active profile. */
    conflictOverridden: string;
    /** Tooltip and accessible text of the "wins" badge: files won and the mods it beats. */
    conflictWinsDetail: (count: number, others: string[]) => string;
    /** Tooltip and accessible text of the "overridden" badge: files lost and the mods that win them. */
    conflictOverriddenDetail: (count: number, winners: string[]) => string;
  };
  /** Discover tab (#73): the curated online catalog. */
  discover: {
    title: string;
    /** Subtitle: entry count and the date of the catalog shown. */
    summary: (count: number, date: string) => string;
    refresh: string;
    refreshing: string;
    loading: string;
    /** Stale banner: the catalog shown is the one saved on `date`. */
    stale: (date: string) => string;
    /** Notice: the catalog shown is the one shipped with this version of the launcher. */
    bundled: string;
    categories: string;
    all: string;
    search: string;
    searchPlaceholder: string;
    sortLabel: string;
    sortUpdated: string;
    sortName: string;
    viewLabel: string;
    gridView: string;
    listView: string;
    /** Accessible name of the results list. */
    results: string;
    /** Live count of the visible entries. */
    resultCount: (count: number) => string;
    noResults: string;
    clearFilters: string;
    inLibrary: string;
    inLibraryDetail: string;
    byAuthor: (author: string) => string;
    version: (version: string) => string;
    updated: (date: string) => string;
    emptyTitle: string;
    emptyCurated: string;
    emptyHowTo: string;
    emptyMeanwhile: string;
    goToLibrary: string;
    retry: string;
    detail: {
      close: string;
      description: string;
      version: string;
      author: string;
      updated: string;
      game: string;
      category: string;
      page: string;
      openPage: string;
      /** Hint under Open page: the host the page opens on, in the default browser. */
      openPageHint: (host: string) => string;
      install: string;
    };
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
    /** Reason the create, rename, delete and save actions are inert while a game session is running. */
    locked: string;
    createTitle: string;
    createHint: string;
    listTitle: string;
    summary: (count: number, activeName: string | null) => string;
    modCount: (count: number) => string;
    active: string;
    selectHint: string;
    orderHint: string;
    conflictsTitle: string;
    /** Live summary of the draft's conflicts; 0 reads as "no conflicts". */
    conflictsSummary: (count: number) => string;
    conflictsHint: string;
    conflictsEmpty: string;
    conflictWinner: string;
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
    adPreference: string;
    adPreferenceHint: string;
    adShow: string;
    adOff: string;
    languageHint: string;
    language: string;
    languageNames: Record<Locale, string>;
    subtitle: string;
    sectionsLabel: string;
    folderTitle: string;
  };
  storage: {
    title: string;
    folderLabel: string;
    folderHint: string;
    openFolder: string;
    openFailed: string;
    totalUsed: string;
    freeSpace: string;
    unavailable: string;
    refresh: string;
    measuring: string;
    breakdownLabel: string;
    categories: Record<StorageCategory, string>;
    categoryHints: Record<StorageCategory, string>;
    measureFailed: string;
    retry: string;
  };
  titleBar: {
    controls: string;
    minimize: string;
    maximize: string;
    restore: string;
    close: string;
  };
  statusBar: {
    steam: string;
    noGame: string;
    setUp: string;
    ready: string;
    version: (version: string) => string;
    versionTitle: string;
  };
  /** Support the project (#94): the sidebar footer entry and the Settings > About row. */
  support: {
    /** Sidebar entry text, its tooltip when collapsed, and the About row label. */
    label: string;
    /** Accessible description of both entries: the page opens outside the launcher. */
    opensInBrowser: string;
    aboutText: string;
    button: string;
    openFailed: string;
  };
  /** Sponsored placements (#104): the visible label, the single action and its description. */
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
  /** Labels of the catalog install button (#74). */
  catalogInstall: {
    install: string;
    /** `percent` is an integer 0-100. */
    downloading: (percent: number) => string;
    verifying: string;
    installing: string;
    installed: string;
    failed: string;
    retry: string;
    cancel: string;
    /** Accessible name of the download progress bar. */
    progressLabel: string;
  };
  errors: Record<ErrorCode, string>;
}
