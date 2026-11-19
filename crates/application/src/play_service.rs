//! Play orchestrator and startup recovery. It sequences ports; every rule it
//! applies (ownership, authorized directory, App ID) lives in the services and
//! the domain it delegates to.
use crate::{
    AppError, BackgroundRunner, DeployError, DeployFiles, DeployMod, DeployService,
    DeployedRepository, Deployer, ModPlanner, PlayError, PlayGame, SessionClock, SessionError,
    SessionEvent, SessionLock, SessionPublisher, SessionRuntime, SessionService, SessionState,
    TrackOutcome, UrlOpener,
};
use crate::ProcessInspector;
use motogp_domain::{launch_url, GameInfo, WatchEvent};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

/// Whether new deploys are allowed.
enum Recovery {
    /// Startup recovery has not run: Play is not accepted yet.
    NotRun,
    Ready,
    /// A cleanup failed; new deploys wait until a retry succeeds.
    Blocked,
}

struct Inner {
    game: Arc<dyn PlayGame>,
    planner: Arc<dyn ModPlanner>,
    deployer: Arc<dyn Deployer>,
    runtime: Arc<dyn SessionRuntime>,
    lock: Arc<dyn SessionLock>,
    runner: Arc<dyn BackgroundRunner>,
    publisher: Arc<dyn SessionPublisher>,
    recovery: Mutex<Recovery>,
    recovered: Mutex<Option<usize>>,
}

/// Starts and finishes game sessions. Cheap to clone (shared state).
#[derive(Clone)]
pub struct PlayService(Arc<Inner>);

impl PlayService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        game: Arc<dyn PlayGame>,
        planner: Arc<dyn ModPlanner>,
        deployer: Arc<dyn Deployer>,
        runtime: Arc<dyn SessionRuntime>,
        lock: Arc<dyn SessionLock>,
        runner: Arc<dyn BackgroundRunner>,
        publisher: Arc<dyn SessionPublisher>,
    ) -> Self {
        PlayService(Arc::new(Inner {
            game,
            planner,
            deployer,
            runtime,
            lock,
            runner,
            publisher,
            recovery: Mutex::new(Recovery::NotRun),
            recovered: Mutex::new(None),
        }))
    }

    /// Cleans a session left deployed by a crash (same ownership and
    /// authorized-directory rules as any cleanup). Must run before the first
    /// `play`. A failure is reported, keeps the pending items and blocks new
    /// deploys until a later cleanup succeeds.
    pub fn recover_on_startup(&self) -> Result<usize, PlayError> {
        let inner = &self.0;
        if !inner.lock.try_acquire() {
            return Err(PlayError::AlreadyRunning);
        }
        let result = inner.try_recover();
        inner.lock.release();
        match result {
            Ok(cleaned) => {
                *relock(&inner.recovery) = Recovery::Ready;
                *relock(&inner.recovered) = Some(cleaned);
                Ok(cleaned)
            }
            Err(error) => {
                inner.block(error.to_string());
                Err(error)
            }
        }
    }

    /// Files removed by the startup recovery; `None` until it has run.
    pub fn recovered_count(&self) -> Option<usize> {
        *relock(&self.0.recovered)
    }

    /// Validates, deploys, opens the game and tracks the session in the
    /// background. The lock is held from here until the cleanup finishes.
    pub fn play(&self, profile_id: &str) -> Result<(), PlayError> {
        let inner = &self.0;
        if !inner.lock.try_acquire() {
            return Err(PlayError::AlreadyRunning);
        }
        match inner.prepare_and_launch(profile_id) {
            Ok(game) => {
                let shared = inner.clone();
                inner
                    .runner
                    .spawn(Box::new(move || shared.run_session(&game)));
                Ok(())
            }
            Err(error) => {
                inner.lock.release();
                Err(error)
            }
        }
    }
}

fn relock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Inner {
    fn try_recover(&self) -> Result<usize, PlayError> {
        if self.deployer.is_clean()? {
            return Ok(0);
        }
        let game = self.game.configured_game()?;
        Ok(self.deployer.clean(&game.mods_dir)?)
    }

    fn block(&self, message: String) {
        *relock(&self.recovery) = Recovery::Blocked;
        self.publisher.report_error(&message);
    }

    /// Everything up to (and including) opening the game. On `Err` nothing of
    /// this attempt is left deployed unless the compensation itself failed,
    /// in which case new deploys are blocked.
    fn prepare_and_launch(&self, profile_id: &str) -> Result<GameInfo, PlayError> {
        let blocked = match &*relock(&self.recovery) {
            Recovery::NotRun => {
                return Err(PlayError::RecoveryPending {
                    detail: "startup recovery has not run".into(),
                })
            }
            Recovery::Ready => false,
            Recovery::Blocked => true,
        };
        // Validate the prerequisites before touching ~mods.
        let game = self.game.configured_game()?;
        launch_url(&game)?;
        if blocked {
            self.retry_blocked_cleanup(&game)?;
        }
        let mods = self.planner.plan(profile_id)?;

        if let Err(error) = self.deployer.deploy(&game.mods_dir, &mods) {
            self.compensate(&game);
            return Err(error.into());
        }
        if let Err(error) = self.runtime.launch(&game) {
            self.compensate(&game);
            return Err(PlayError::Session(error));
        }
        Ok(game)
    }

    fn retry_blocked_cleanup(&self, game: &GameInfo) -> Result<(), PlayError> {
        match self.deployer.clean(&game.mods_dir) {
            Ok(_) => {
                *relock(&self.recovery) = Recovery::Ready;
                Ok(())
            }
            Err(error) => {
                let detail = error.to_string();
                *relock(&self.recovery) = Recovery::Blocked;
                Err(PlayError::RecoveryPending { detail })
            }
        }
    }

    /// Removes only the files of the failed attempt (the manifest tracks
    /// nothing else at this point). A failure keeps the manifest and blocks.
    fn compensate(&self, game: &GameInfo) {
        if let Err(error) = self.deployer.clean(&game.mods_dir) {
            self.block(error.to_string());
        }
    }

    /// Background part of a session: publish `started`, then clean and
    /// publish the final event.
    fn run_session(&self, game: &GameInfo) {
        let mut terminal = None;
        let outcome = self.runtime.track(&game.root, &mut |event| match event {
            WatchEvent::Started => self.publisher.publish(SessionEvent {
                state: SessionState::Started,
                cleaned: 0,
            }),
            other => terminal = Some(other),
        });
        match (outcome, terminal) {
            (Ok(TrackOutcome::Finished), Some(event)) => self.finish(game, event),
            (Err(error), _) => self.tracking_failed(&error),
            // Cancelled (or no terminal event): the session is not over, so
            // nothing is cleaned and the lock stays held.
            _ => {}
        }
    }

    fn finish(&self, game: &GameInfo, event: WatchEvent) {
        let state = match event {
            WatchEvent::Started => return,
            WatchEvent::Ended => SessionState::Ended,
            WatchEvent::NeverStarted => SessionState::NeverStarted,
        };
        match self.deployer.clean(&game.mods_dir) {
            Ok(cleaned) => {
                self.lock.release();
                self.publisher.publish(SessionEvent { state, cleaned });
            }
            Err(error) => {
                self.block(error.to_string());
                self.lock.release();
            }
        }
    }

    /// The watcher died: the files stay deployed (the game may still be
    /// running) and the next Play retries the cleanup first.
    fn tracking_failed(&self, error: &AppError) {
        self.block(format!("could not follow the game session: {error}"));
        self.lock.release();
    }
}

impl<R, F> Deployer for DeployService<R, F>
where
    R: DeployedRepository + Send + Sync,
    F: DeployFiles + Send + Sync,
{
    fn deploy(&self, mods_dir: &Path, mods: &[DeployMod]) -> Result<Vec<String>, DeployError> {
        DeployService::deploy(self, mods_dir, mods)
    }

    fn clean(&self, mods_dir: &Path) -> Result<usize, DeployError> {
        DeployService::clean(self, mods_dir)
    }

    fn is_clean(&self) -> Result<bool, DeployError> {
        DeployService::is_clean(self)
    }
}

impl<O, P, C> SessionRuntime for SessionService<O, P, C>
where
    O: UrlOpener + Send + Sync,
    P: ProcessInspector + Send + Sync,
    C: SessionClock + Send + Sync,
{
    fn launch(&self, game: &GameInfo) -> Result<(), SessionError> {
        SessionService::launch(self, game)
    }

    fn track(
        &self,
        root: &Path,
        emit: &mut dyn FnMut(WatchEvent),
    ) -> Result<TrackOutcome, AppError> {
        SessionService::track(self, root, || false, |event| emit(event))
    }
}
