import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Fixed port consumed by `devUrl` in src-tauri/tauri.conf.json.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
});
