//! IPC data transfer objects. Fields are snake_case on the wire; the
//! JS-side command arguments are camelCase (handled by `#[tauri::command]`).
use motogp_application::{RecoveryState, RecoveryStatus, SessionEvent, SessionState};
use motogp_domain::{Config, GameInfo, ModEntry, Profile};
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
