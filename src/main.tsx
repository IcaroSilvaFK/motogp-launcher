// Composition root: the ONLY file allowed to wire ui, application and
// infrastructure together (Tauri adapters are injected into the controller here).
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import {
  createCatalogController,
  createLauncherController,
  createUpdateController,
  loadLocale,
  saveLocale,
  selectSessionClean,
} from './application';
import {
  createLocalStorageAdPreferenceStore,
  createLocalStorageDiscoverViewStore,
  createTauriAdProvider,
  createLocalStorageLocaleStore,
  createLocalStorageNavStore,
  createTauriAppInfo,
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

const controller = createLauncherController({
  gateway,
  picker: createTauriPicker(),
  session: createTauriSessionEvents(),
  drop: createTauriDropEvents(),
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
      navStore={createLocalStorageNavStore()}
      windowControls={createTauriWindowControls()}
      catalog={catalog}
      discoverViewStore={createLocalStorageDiscoverViewStore()}
      adPreferenceStore={createLocalStorageAdPreferenceStore()}
      // Sponsored placements (#104): slots render only while the preference above allows them.
      adProvider={createTauriAdProvider(() => activeLocale)}
    />
  </StrictMode>,
);
