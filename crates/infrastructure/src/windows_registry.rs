//! Windows adapter: reads the Steam install path from the registry
//! (`HKCU\Software\Valve\Steam` `SteamPath`, falling back to
//! `HKLM\SOFTWARE\WOW6432Node\Valve\Steam` `InstallPath`). The precedence logic
//! is unit-tested through an injectable reader; the real registry access is
//! verified by a manual smoke only (see docs/validation-workflow.md).
use crate::SteamLibraries;
use std::path::PathBuf;

/// Reads a string value from a registry location; `None` when absent/unreadable.
pub trait RegistryReader {
    fn read(&self, hive: Hive, subkey: &str, value: &str) -> Option<String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

const HKCU_KEY: &str = r"Software\Valve\Steam";
const HKCU_VALUE: &str = "SteamPath";
const HKLM_KEY: &str = r"SOFTWARE\WOW6432Node\Valve\Steam";
const HKLM_VALUE: &str = "InstallPath";

/// HKCU first; falls back to HKLM when the HKCU value is missing or empty.
pub fn steam_path_from(reader: &impl RegistryReader) -> Option<PathBuf> {
    let non_empty = |raw: Option<String>| {
        raw.map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    };
    non_empty(reader.read(Hive::CurrentUser, HKCU_KEY, HKCU_VALUE))
        .or_else(|| non_empty(reader.read(Hive::LocalMachine, HKLM_KEY, HKLM_VALUE)))
}

struct WinRegistry;

impl RegistryReader for WinRegistry {
    fn read(&self, hive: Hive, subkey: &str, value: &str) -> Option<String> {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;

        let root = match hive {
            Hive::CurrentUser => HKEY_CURRENT_USER,
            Hive::LocalMachine => HKEY_LOCAL_MACHINE,
        };
        RegKey::predef(root)
            .open_subkey(subkey)
            .ok()?
            .get_value(value)
            .ok()
    }
}

/// Steam install path from the registry; `None` when Steam is not installed.
pub fn read_steam_path() -> Option<PathBuf> {
    steam_path_from(&WinRegistry)
}

impl SteamLibraries {
    /// Locator backed by the real Windows registry.
    pub fn from_registry() -> Self {
        SteamLibraries::new(read_steam_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct FakeRegistry(HashMap<(Hive, String, String), String>);

    impl FakeRegistry {
        fn with(mut self, hive: Hive, key: &str, value: &str, data: &str) -> Self {
            self.0.insert((hive, key.into(), value.into()), data.into());
            self
        }
    }

    impl RegistryReader for FakeRegistry {
        fn read(&self, hive: Hive, subkey: &str, value: &str) -> Option<String> {
            self.0.get(&(hive, subkey.into(), value.into())).cloned()
        }
    }

    #[test]
    fn hkcu_wins_over_hklm() {
        let reg = FakeRegistry::default()
            .with(Hive::CurrentUser, HKCU_KEY, HKCU_VALUE, "C:/User/Steam")
            .with(
                Hive::LocalMachine,
                HKLM_KEY,
                HKLM_VALUE,
                r"C:\Machine\Steam",
            );
        assert_eq!(steam_path_from(&reg), Some(PathBuf::from("C:/User/Steam")));
    }

    #[test]
    fn falls_back_to_hklm_when_hkcu_is_missing() {
        let reg = FakeRegistry::default().with(
            Hive::LocalMachine,
            HKLM_KEY,
            HKLM_VALUE,
            r"C:\Machine\Steam",
        );
        assert_eq!(
            steam_path_from(&reg),
            Some(PathBuf::from(r"C:\Machine\Steam"))
        );
    }

    #[test]
    fn falls_back_to_hklm_when_hkcu_is_blank() {
        let reg = FakeRegistry::default()
            .with(Hive::CurrentUser, HKCU_KEY, HKCU_VALUE, "   ")
            .with(
                Hive::LocalMachine,
                HKLM_KEY,
                HKLM_VALUE,
                r"C:\Machine\Steam",
            );
        assert_eq!(
            steam_path_from(&reg),
            Some(PathBuf::from(r"C:\Machine\Steam"))
        );
    }

    #[test]
    fn no_steam_anywhere_is_none() {
        assert_eq!(steam_path_from(&FakeRegistry::default()), None);
    }
}
