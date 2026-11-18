//! Bootstrap do app Tauri. Excluído da cobertura (ver `scripts/check-rust-coverage.mjs`
//! e `docs/workflow-validacao.md`): só roda com janela e WebView2 reais; a
//! verificação é o smoke `pnpm tauri dev`.

/// Inicia o aplicativo Tauri.
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("erro ao executar o MotoGP Launcher");
}
