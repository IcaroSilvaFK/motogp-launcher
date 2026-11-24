//! IPC data transfer objects. Fields are snake_case on the wire; the
//! JS-side command arguments are camelCase (handled by `#[tauri::command]`).
use motogp_application::{
    DeploySession, RecoveryState, RecoveryStatus, SessionEvent, SessionState,
};
use motogp_domain::{
    Config, FileConflict, GameInfo, ModEntry, Profile, StorageCategory, StorageUsage,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigDto {
    pub game_dir: Option<String>,
    pub active_profile: Option<String>,
}

impl From<Config> for ConfigDto {
    fn from(config: Config) -> Self {
        ConfigDto {
            game_dir: config.game_dir.map(|p| p.to_string_lossy().into_owned()),
            active_profile: config.active_profile,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GameInfoDto {
    pub name: String,
    pub root: String,
    pub mods_dir: String,
    pub app_id: Option<u32>,
}

impl From<GameInfo> for GameInfoDto {
    fn from(game: GameInfo) -> Self {
        GameInfoDto {
            name: game.name,
            root: game.root.to_string_lossy().into_owned(),
            mods_dir: game.mods_dir.to_string_lossy().into_owned(),
            app_id: game.app_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModDto {
    pub id: String,
    pub name: String,
    pub files: Vec<String>,
    pub size_bytes: u64,
}

impl From<ModEntry> for ModDto {
    fn from(entry: ModEntry) -> Self {
        ModDto {
            id: entry.id,
            name: entry.name,
            files: entry.files,
            size_bytes: entry.size_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProfileDto {
    pub id: String,
    pub name: String,
    pub mod_ids: Vec<String>,
}

impl From<Profile> for ProfileDto {
    fn from(profile: Profile) -> Self {
        ProfileDto {
            id: profile.id,
            name: profile.name,
            mod_ids: profile.mod_ids,
        }
    }
}

/// A file shipped by two or more mods of a load order; `winner` loads last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConflictDto {
    pub path: String,
    pub mod_ids: Vec<String>,
    pub winner: String,
}

impl From<FileConflict> for ConflictDto {
    fn from(conflict: FileConflict) -> Self {
        ConflictDto {
            winner: conflict.winner().to_string(),
            path: conflict.path,
            mod_ids: conflict.mod_ids,
        }
    }
}

/// Payload of the `session` event: `state` is `started`, `ended` or `never-started`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionEventDto {
    pub state: &'static str,
    pub cleaned: usize,
}

impl From<SessionEvent> for SessionEventDto {
    fn from(event: SessionEvent) -> Self {
        let state = match event.state {
            SessionState::Started => "started",
            SessionState::Ended => "ended",
            SessionState::NeverStarted => "never-started",
        };
        SessionEventDto {
            state,
            cleaned: event.cleaned,
        }
    }
}

/// Payload of the `session-error` event (the visible error channel).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionErrorDto {
    pub message: String,
}

/// Result of `recovery_status`: `state` is `not-run`, `recovered`,
/// `game-running` or `failed`. The frontend reads it after subscribing to the
/// events, so a failure at startup is never lost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryStatusDto {
    pub state: &'static str,
    pub error: Option<String>,
    pub cleaned_count: usize,
}

impl From<RecoveryStatus> for RecoveryStatusDto {
    fn from(status: RecoveryStatus) -> Self {
        let state = match status.state {
            RecoveryState::NotRun => "not-run",
            RecoveryState::Recovered => "recovered",
            RecoveryState::GameRunning => "game-running",
            RecoveryState::Failed => "failed",
        };
        RecoveryStatusDto {
            state,
            error: status.error,
            cleaned_count: status.cleaned_count,
        }
    }
}

/// Result of `deploy_session`: `state` is `none`, `current`, `stale` or
/// `vanished`. `mods_dir` is the folder the manifest records, only for
/// `stale` and `vanished`; `tracked` counts the files and pending entries of
/// `current` and `stale` (0 otherwise).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeploySessionDto {
    pub state: &'static str,
    pub mods_dir: Option<String>,
    pub tracked: usize,
}

impl From<DeploySession> for DeploySessionDto {
    fn from(session: DeploySession) -> Self {
        let dir = |p: std::path::PathBuf| Some(p.to_string_lossy().into_owned());
        match session {
            DeploySession::None => DeploySessionDto {
                state: "none",
                mods_dir: None,
                tracked: 0,
            },
            DeploySession::Current { tracked } => DeploySessionDto {
                state: "current",
                mods_dir: None,
                tracked,
            },
            DeploySession::Stale { mods_dir, tracked } => DeploySessionDto {
                state: "stale",
                mods_dir: dir(mods_dir),
                tracked,
            },
            DeploySession::Vanished { mods_dir } => DeploySessionDto {
                state: "vanished",
                mods_dir: dir(mods_dir),
                tracked: 0,
            },
        }
    }
}

/// One row of the storage breakdown; `category` is `mods`, `launcher_data`
/// or `other`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorageCategoryDto {
    pub category: &'static str,
    pub bytes: u64,
}

/// Result of `get_storage_usage`. Every category is present, in the fixed
/// order `mods`, `launcher_data`, `other`; `free_bytes` is null when unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StorageUsageDto {
    pub root: String,
    pub total_bytes: u64,
    pub free_bytes: Option<u64>,
    pub categories: Vec<StorageCategoryDto>,
}

impl From<StorageUsage> for StorageUsageDto {
    fn from(usage: StorageUsage) -> Self {
        StorageUsageDto {
            total_bytes: usage.total_bytes(),
            root: usage.root.to_string_lossy().into_owned(),
            free_bytes: usage.free_bytes,
            categories: usage
                .categories
                .iter()
                .map(|(category, bytes)| StorageCategoryDto {
                    category: match category {
                        StorageCategory::Mods => "mods",
                        StorageCategory::LauncherData => "launcher_data",
                        StorageCategory::Other => "other",
                    },
                    bytes: *bytes,
                })
                .collect(),
        }
    }
}

/// Game artwork as a `data:<mime>;base64,<bytes>` URL, so the WebView can
/// show it without a file or network permission (CSP `img-src 'self' data:`).
pub fn artwork_data_url(artwork: &motogp_application::Artwork) -> String {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    format!(
        "data:{};base64,{}",
        artwork.mime.as_str(),
        STANDARD.encode(&artwork.bytes)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn artwork_data_url_embeds_the_mime_and_base64_bytes() {
        let art = motogp_application::Artwork {
            bytes: vec![1, 2, 3],
            mime: motogp_application::ImageMime::Png,
        };
        assert_eq!(artwork_data_url(&art), "data:image/png;base64,AQID");
    }

    fn dto(session: DeploySession) -> serde_json::Value {
        serde_json::to_value(DeploySessionDto::from(session)).unwrap()
    }

    #[test]
    fn deploy_session_dto_maps_every_state_to_snake_case_fields() {
        assert_eq!(
            dto(DeploySession::None),
            serde_json::json!({ "state": "none", "mods_dir": null, "tracked": 0 })
        );
        assert_eq!(
            dto(DeploySession::Current { tracked: 3 }),
            serde_json::json!({ "state": "current", "mods_dir": null, "tracked": 3 })
        );
        assert_eq!(
            dto(DeploySession::Stale {
                mods_dir: PathBuf::from("old/~mods"),
                tracked: 2
            }),
            serde_json::json!({ "state": "stale", "mods_dir": "old/~mods", "tracked": 2 })
        );
        assert_eq!(
            dto(DeploySession::Vanished {
                mods_dir: PathBuf::from("old/~mods")
            }),
            serde_json::json!({ "state": "vanished", "mods_dir": "old/~mods", "tracked": 0 })
        );
    }

    #[test]
    fn storage_usage_dto_keeps_the_category_order_and_snake_case_fields() {
        use motogp_domain::{StorageCategory, StorageUsage};
        let usage = StorageUsage::from_entries(
            PathBuf::from("data"),
            [(StorageCategory::Other, 5), (StorageCategory::Mods, 300)],
            None,
        );
        assert_eq!(
            serde_json::to_value(StorageUsageDto::from(usage)).unwrap(),
            serde_json::json!({
                "root": "data",
                "total_bytes": 305,
                "free_bytes": null,
                "categories": [
                    { "category": "mods", "bytes": 300 },
                    { "category": "launcher_data", "bytes": 0 },
                    { "category": "other", "bytes": 5 }
                ]
            })
        );
    }
}
