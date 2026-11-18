//! Tauri app bootstrap. Excluded from coverage (see `scripts/check-rust-coverage.mjs`
//! and `docs/workflow-validacao.md`): it only runs with a real window and WebView2;
//! it is verified by the `pnpm tauri dev` smoke test.

/// Starts the Tauri application.
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running MotoGP Launcher");
}
