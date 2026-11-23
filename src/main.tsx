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

// Fire-and-forget: the check never blocks the launcher; failures become a recoverable notice.
void updater.check();

createRoot(container).render(
  <StrictMode>
    <App
      controller={controller}
      locale={loadLocale(localeStore)}
      onLocaleChange={(locale) => saveLocale(localeStore, locale)}
      updater={updater}
    />
  </StrictMode>,
);
