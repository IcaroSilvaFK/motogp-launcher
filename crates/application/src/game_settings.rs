//! Use cases that configure the game folder, on top of `GameService` and the
//! `ConfigRepository` port.
use crate::{
    AppError, ConfigRepository, GameError, GameFiles, GameService, PlayError, PlayGame,
    SteamLocator,
};
use motogp_domain::{Config, GameInfo};
use std::path::Path;

pub struct GameSettings<C, L: SteamLocator, F: GameFiles> {
    config: C,
    games: GameService<L, F>,
}

impl<C: ConfigRepository, L: SteamLocator, F: GameFiles> GameSettings<C, L, F> {
    pub fn new(config: C, games: GameService<L, F>) -> Self {
        Self { config, games }
    }

    /// The stored configuration (missing storage yields the default).
    pub fn config(&self) -> Result<Config, AppError> {
        self.config.load()
    }

    /// Games found automatically (best effort, may be empty).
    pub fn detect_games(&self) -> Result<Vec<GameInfo>, GameError> {
        self.games.detect_games()
    }

    /// Inspects `path` first and saves it only when it is a valid game; any
    /// other configuration field is preserved.
    pub fn set_game_dir(&self, path: &Path) -> Result<GameInfo, GameError> {
        let game = self.games.inspect_root(path)?;
        let current = self.config.load()?;
        self.config.save(&Config {
            game_dir: Some(path.to_path_buf()),
            ..current
        })?;
        Ok(game)
    }

    /// `None` only when no folder is configured; an inspection failure of a
    /// configured folder is an error, never hidden as `None`.
    pub fn get_game(&self) -> Result<Option<GameInfo>, GameError> {
        match self.config.load()?.game_dir {
            None => Ok(None),
            Some(dir) => self.games.inspect_root(&dir).map(Some),
        }
    }
}

impl<C, L, F> PlayGame for GameSettings<C, L, F>
where
    C: ConfigRepository + Send + Sync,
    L: SteamLocator + Send + Sync,
    F: GameFiles + Send + Sync,
{
    fn configured_game(&self) -> Result<GameInfo, PlayError> {
        self.get_game()?.ok_or(PlayError::GameNotConfigured)
    }
}
