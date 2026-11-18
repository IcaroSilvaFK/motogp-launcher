use crate::{AppError, ConfigRepository};
use motogp_domain::Config;

/// Use cases for the user configuration, built on the `ConfigRepository` port.
pub struct ConfigService<R: ConfigRepository> {
    repository: R,
}

impl<R: ConfigRepository> ConfigService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub fn load(&self) -> Result<Config, AppError> {
        todo!("red")
    }

    pub fn save(&self, _config: &Config) -> Result<(), AppError> {
        todo!("red")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeRepository {
        load_result: Result<Config, AppError>,
        save_result: Result<(), AppError>,
        saved: RefCell<Vec<Config>>,
    }

    impl FakeRepository {
        fn ok(config: Config) -> Self {
            Self {
                load_result: Ok(config),
                save_result: Ok(()),
                saved: RefCell::new(Vec::new()),
            }
        }

        fn failing(error: AppError) -> Self {
            Self {
                load_result: Err(error.clone()),
                save_result: Err(error),
                saved: RefCell::new(Vec::new()),
            }
        }
    }

    impl ConfigRepository for FakeRepository {
        fn load(&self) -> Result<Config, AppError> {
            self.load_result.clone()
        }

        fn save(&self, config: &Config) -> Result<(), AppError> {
            self.saved.borrow_mut().push(config.clone());
            self.save_result.clone()
        }
    }

    fn corrupted() -> AppError {
        AppError::Corrupted {
            location: "config.json".into(),
            detail: "expected value".into(),
        }
    }

    #[test]
    fn config_repository_error_is_propagated() {
        let service = ConfigService::new(FakeRepository::failing(corrupted()));
        assert_eq!(service.load(), Err(corrupted()));
        assert_eq!(service.save(&Config::default()), Err(corrupted()));
    }

    #[test]
    fn load_returns_what_the_repository_provides() {
        let config = Config {
            game_dir: Some("Game Dir".into()),
            active_profile: Some("p1".into()),
        };
        let service = ConfigService::new(FakeRepository::ok(config.clone()));
        assert_eq!(service.load(), Ok(config));
    }

    #[test]
    fn save_delegates_the_given_config_to_the_repository() {
        let config = Config {
            game_dir: None,
            active_profile: Some("p2".into()),
        };
        let service = ConfigService::new(FakeRepository::ok(Config::default()));
        assert_eq!(service.save(&config), Ok(()));
        assert_eq!(service.repository.saved.borrow().as_slice(), &[config]);
    }
}
