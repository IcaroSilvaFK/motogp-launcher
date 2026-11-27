import type {
  ErrorCode,
  Locale,
  PublishErrorCode,
  PublishFormErrorKey,
  ReportReason,
  StorageCategory,
  UpdateErrorKind,
  VersionStatus,
} from '../../domain';

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
    /** Full mod page inside Discover (#115). */
    modPage: {
      /** Visible label of the back button. */
      back: string;
      /** Accessible name of the back button. */
      backTo: string;
      breadcrumb: string;
      /** Accessible name of the stats strip. */
      stats: string;
      size: string;
      sha256: string;
      details: string;
      /** Name of the gallery region. */
      gallery: string;
      /** Name of the thumbnail group. */
      thumbnails: string;
      /** Alt text of a screenshot: the mod title plus its position. */
      screenshotAlt: (title: string, index: number, total: number) => string;
      /** Name of a thumbnail button. */
      showScreenshot: (index: number, total: number) => string;
      /** Tooltip of the main image, which opens the viewer. */
      viewFullSize: string;
      /** Name of the lightbox dialog. */
      viewer: (title: string) => string;
      previous: string;
      next: string;
      closeViewer: string;
      /** Lightbox position, e.g. "2 / 3". */
      counter: (index: number, total: number) => string;
      /** Title of the author card. */
      author: string;
      /** Count of the author's mods in the catalog, this one included. */
      authorMods: (count: number) => string;
      /** Action of the author card: back to the list searched by the author. */
      moreFromAuthor: (author: string) => string;
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
    autoEnable: string;
    autoEnableHint: string;
    adShow: string;
    adOff: string;
    /** Anonymous usage sharing (#120): a switch, on unless the user turns it off. */
    shareUsage: string;
    shareUsageHint: string;
    shareUsageOn: string;
    shareUsageOff: string;
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
    logsLabel: string;
    logsHint: string;
    openLogs: string;
    openLogsFailed: string;
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
    diagnosticsLabel: string;
    diagnosticsHint: string;
    copyDiagnostics: string;
    copied: string;
    copyFailed: string;
  };
  /** Settings > Community: the Discord account row (#122). */
  community: {
    title: string;
    accountLabel: string;
    signedOutHint: string;
    signedInHint: string;
    signIn: string;
    signOut: string;
    waiting: string;
    waitingHint: string;
    offline: string;
    loading: string;
    dismiss: string;
    errors: {
      denied: string;
      timeout: string;
      failed: string;
    };
  };
  /** Community stars (#119): the mod page button, the Discover counts and sort. */
  stars: {
    star: string;
    count: (stars: number) => string;
    sort: string;
    signInPrompt: string;
    notNow: string;
    failed: string;
    /** The star was refused because the community account is suspended (#121). */
    banned: string;
  };
  /** Mod page > Comments (#118). */
  comments: {
    title: string;
    /** The count next to the heading; `more` while further pages exist ("20+"). */
    count: (loaded: number, more: boolean) => string;
    loading: string;
    emptyTitle: string;
    emptyHint: string;
    offline: string;
    retry: string;
    loadMore: string;
    edited: string;
    /** Chip on the signed-in member's own comments. */
    you: string;
    composerLabel: string;
    placeholder: string;
    /** Visible character counter under a draft. */
    counter: (used: number, max: number) => string;
    tooLong: (over: number) => string;
    shortcutHint: string;
    post: string;
    signIn: string;
    signInHint: string;
    accountOffline: string;
    edit: string;
    editLabel: string;
    save: string;
    cancel: string;
    delete: string;
    confirmDelete: string;
    dismiss: string;
    /** Report someone else's comment (#121). */
    report: string;
    reportTitle: string;
    reportReasonsLegend: string;
    reportReasons: Record<ReportReason, string>;
    reportDetails: string;
    reportDetailsPlaceholder: string;
    reportSend: string;
    reported: string;
    reportedStatus: string;
    reportInvalid: string;
    /** Shown in place of the composer once the server says the account is suspended. */
    suspendedTitle: string;
    errors: {
      offline: string;
      signed_out: string;
      rate_limited: (seconds: number) => string;
      invalid: string;
      not_found: string;
      banned: string;
      failed: string;
    };
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
  /** Publishing a mod (#130): translations for the typed publish error codes and the form rules. */
  publish: {
    errors: Record<PublishErrorCode, string>;
    validation: Record<PublishFormErrorKey, string>;
    /** The Discover header button and the publish page title. */
    open: string;
    subtitle: (name: string) => string;
    back: string;
    backTo: string;
    tabsLabel: string;
    tabPublish: string;
    tabMyMods: string;
    sectionDetails: string;
    sectionFiles: string;
    title: string;
    modId: string;
    modIdHint: string;
    description: string;
    descriptionHint: string;
    longDescription: string;
    longDescriptionHint: string;
    category: string;
    game: string;
    version: string;
    versionHint: string;
    file: string;
    fileHint: string;
    noFile: string;
    chooseFile: string;
    changeFile: string;
    screenshots: string;
    screenshotsHint: string;
    addScreenshots: string;
    screenshotCount: (count: number, max: number) => string;
    removeScreenshot: (name: string) => string;
    ownership: string;
    submit: string;
    submitting: string;
    fixErrors: (count: number) => string;
    progressLabel: string;
    preparing: string;
    hashing: string;
    uploading: (sent: string, total: string) => string;
    finishing: string;
    cancel: string;
    cancelled: string;
    retry: string;
    dismiss: string;
    doneTitle: string;
    doneBody: (title: string, version: string) => string;
    viewMyMods: string;
    publishAnother: string;
    myMods: {
      loading: string;
      emptyTitle: string;
      emptyHint: string;
      refresh: string;
      refreshing: string;
      retry: string;
      versionLabel: (version: string) => string;
      versions: (count: number) => string;
      uploaded: (date: string) => string;
      reason: (reason: string) => string;
      status: Record<VersionStatus, string>;
    };
  };
  notices: {
    modInstalled: (name: string) => string;
    modInstalledEnabled: (name: string) => string;
    modInstalledConflict: (name: string, other: string) => string;
    undo: string;
    openLibrary: string;
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
    installedEnabled: string;
    failed: string;
    retry: string;
    cancel: string;
    /** Accessible name of the download progress bar. */
    progressLabel: string;
  };
  /**
   * Download counts and the 30-day chart (#120). `count` drives the plural; `formatted` is the same
   * number already formatted for the UI locale. Dates arrive formatted too.
   */
  downloads: {
    /** Discover card meta line: "1,234 downloads". */
    count: (count: number, formatted: string) => string;
    /** Label of the mod page stats cell. */
    label: string;
    /** Secondary line of the stats cell. */
    last30Days: (formatted: string) => string;
    chartTitle: string;
    /** Text summary of the chart (its accessible name): total in the window and the busiest day. */
    chartSummary: (total: number, formatted: string, peak: { count: string; date: string } | null) => string;
    /** Tooltip of one bar. */
    bar: (date: string, count: number, formatted: string) => string;
  };
  errors: Record<ErrorCode, string>;
}
