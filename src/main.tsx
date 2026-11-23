// Composition root: the ONLY file allowed to wire ui, application and
// infrastructure together (Tauri adapters are injected into the controller here).
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import {
  createLauncherController,
  createUpdateController,
  loadLocale,
  saveLocale,
  selectSessionClean,
} from './application';
import {
  createLocalStorageLocaleStore,
  createTauriAppInfo,
  createTauriDropEvents,
  createTauriGateway,
  createTauriPicker,
  createTauriSessionEvents,
  createTauriUpdateService,
} from './infrastructure';
import { App } from './ui/App';

const container = document.getElementById('root');
if (!container) {
  throw new Error('Root element #root not found');
}

const controller = createLauncherController({
  gateway: createTauriGateway(),
  picker: createTauriPicker(),
  session: createTauriSessionEvents(),
  drop: createTauriDropEvents(),
});

const localeStore = createLocalStorageLocaleStore();

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
      locale={loadLocale(localeStore)}
      onLocaleChange={(locale) => saveLocale(localeStore, locale)}
      updater={updater}
      appInfo={createTauriAppInfo()}
    />
  </StrictMode>,
);
