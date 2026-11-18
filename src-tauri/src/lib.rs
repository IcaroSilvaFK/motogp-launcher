//! Camada de interface (Tauri): bootstrap, comandos finos, conversão de DTOs
//! e mapeamento de erros. A composição dos adapters acontece aqui.

mod bootstrap;

pub use bootstrap::run;
