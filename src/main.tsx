// Composition root: the ONLY file allowed to wire ui, application and
// infrastructure together (Tauri adapters are injected into the controller here).
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { createLauncherController } from './application';
import {
  createTauriDropEvents,
  createTauriGateway,
  createTauriPicker,
  createTauriSessionEvents,
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

createRoot(container).render(
  <StrictMode>
    <App controller={controller} />
  </StrictMode>,
);
