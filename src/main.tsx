// Composition root: the ONLY file allowed to wire ui, application and
// infrastructure together (Tauri adapters are injected into the controller here).
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import {
  createCatalogController,
  createLauncherController,
  createUpdateController,
  createUsagePreference,
  loadLocale,
  reportCatalogDownload,
  saveLocale,
  selectSessionClean,
} from './application';
import {
  createLocalStorageAdPreferenceStore,
  createLocalStorageAutoEnableStore,
  createLocalStorageDiscoverViewStore,
  createTauriAdProvider,
  createTauriCommunityAccount,
  createTauriModStats,
  createTauriModComments,
  createTauriModPublisher,
  createTauriPublishPicker,
  createLocalStorageLocaleStore,
  createLocalStorageNavStore,
  createLocalStorageUsagePreferenceStore,
  createTauriDownloadStats,
  createTauriAppInfo,
  createTauriDiagnostics,
  createTauriDropEvents,
  createTauriGateway,
  createTauriPicker,
  createTauriSessionEvents,
  createTauriUpdateService,
  createTauriWindowControls,
} from './infrastructure';
import { App } from './ui/App';

const container = document.getElementById('root');
if (!container) {
  throw new Error('Root element #root not found');
}

const gateway = createTauriGateway();

// Download counts (#120): one port for the counts and the install report, and the live usage choice
// shared by the Settings switch and the report (an opt-out applies at once, even if it cannot be saved).
const downloadStats = createTauriDownloadStats();
const usagePreference = createUsagePreference(createLocalStorageUsagePreferenceStore());

const controller = createLauncherController({
  gateway,
  picker: createTauriPicker(),
  session: createTauriSessionEvents(),
  drop: createTauriDropEvents(),
  // Fire-and-forget after a successful catalog install only; never awaited, failures are silent.
  onCatalogInstalled: (id) => reportCatalogDownload({ stats: downloadStats, preference: usagePreference }, id),
});

// Discover tab (#73): loads the catalog on the first visit only.
const catalog = createCatalogController({ gateway });

const localeStore = createLocalStorageLocaleStore();
// The active UI locale, also sent to `get_ad` so sponsored copy is localized (#123).
let activeLocale = loadLocale(localeStore);

const updater = createUpdateController({
  service: createTauriUpdateService(),
  isSessionClean: () => selectSessionClean(controller.getState()),
});

// Fire-and-forget: the check never blocks the launcher; failures are only logged, never shown.
void updater.check({ background: true });

createRoot(container).render(
  <StrictMode>
    <App
      controller={controller}
      locale={activeLocale}
      onLocaleChange={(locale) => {
        activeLocale = locale;
        saveLocale(localeStore, locale);
      }}
      updater={updater}
      appInfo={createTauriAppInfo()}
      diagnostics={createTauriDiagnostics()}
      navStore={createLocalStorageNavStore()}
      windowControls={createTauriWindowControls()}
      catalog={catalog}
      discoverViewStore={createLocalStorageDiscoverViewStore()}
      adPreferenceStore={createLocalStorageAdPreferenceStore()}
      autoEnableStore={createLocalStorageAutoEnableStore()}
      // Sponsored placements (#104): slots render only while the preference above allows them.
      adProvider={createTauriAdProvider(() => activeLocale)}
      communityAccount={createTauriCommunityAccount()}
      // Community stars (#119): one cached store per app, shared by Discover and the mod page.
      modStats={createTauriModStats()}
      modComments={createTauriModComments()}
      downloadStats={downloadStats}
      usagePreference={usagePreference}
      // Mod hosting (#130): publish form and "My mods".
      modPublisher={createTauriModPublisher()}
      publishPicker={createTauriPublishPicker()}
    />
  </StrictMode>,
);
