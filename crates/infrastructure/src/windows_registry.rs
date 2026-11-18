//! Windows adapter: reads `HKCU\Software\Valve\Steam` value `SteamPath`.
//! Verified by a manual smoke only (see docs/workflow-validacao.md), never by
//! automated tests, because it depends on the machine's registry.
use crate::SteamLibraries;
use std::path::PathBuf;

/// Steam install path from the registry; `None` when Steam is not installed.
pub fn read_steam_path() -> Option<PathBuf> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Valve\Steam")
        .ok()?;
    let path: String = key.get_value("SteamPath").ok()?;
    let path = path.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

impl SteamLibraries {
    /// Locator backed by the real Windows registry.
    pub fn from_registry() -> Self {
        SteamLibraries::new(read_steam_path())
    }
}
