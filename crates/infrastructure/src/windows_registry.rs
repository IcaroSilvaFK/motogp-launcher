//! Windows adapter: reads `HKCU\Software\Valve\Steam` value `SteamPath`.
//! Verified by a manual smoke only (see docs/workflow-validacao.md), never by
//! automated tests, because it depends on the machine's registry.
use crate::SteamLibraries;
use std::path::PathBuf;

/// Steam install path from the registry; `None` when Steam is not installed.
pub fn read_steam_path() -> Option<PathBuf> {
    todo!()
}

impl SteamLibraries {
    /// Locator backed by the real Windows registry.
    pub fn from_registry() -> Self {
        SteamLibraries::new(read_steam_path())
    }
}
