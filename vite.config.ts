import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// Fixed port consumed by `devUrl` in src-tauri/tauri.conf.json.
// The watcher ignores .worktrees, coverage and target: agents write coverage
// HTML inside .worktrees/<n>, and that used to reload the running dev app.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/.worktrees/**', '**/coverage/**', '**/target/**'] },
  },
});
