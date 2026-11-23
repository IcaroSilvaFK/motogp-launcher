//! Play orchestrator and startup recovery. It sequences ports; every rule it
//! applies (ownership, authorized directory, App ID) lives in the services and
//! the domain it delegates to.
use crate::ProcessInspector;
use crate::{
    AppError, BackgroundRunner, DeployError, DeployFiles, DeployMod, DeployService, DeploySession,
    DeployedRepository, Deployer, ModPlanner, PlayError, PlayGame, RecoveryState, RecoveryStatus,
    SessionClock, SessionError, SessionEvent, SessionLock, SessionPublisher, SessionRuntime,
    SessionService, SessionState, TrackOutcome, UrlOpener,
};
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
    /// The game of a previous run is still running and being tracked.
    GameRunning,
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
    status: Mutex<RecoveryStatus>,
}

/// Holds the session lock and releases it on drop, also while unwinding. A
/// panic, or a worker that never started, leaves the cleanup pending (new
/// deploys blocked) instead of leaving the lock stuck or the files forgotten.
struct LockGuard {
    inner: Arc<Inner>,
    /// `false` while the guard travels to a worker that has not begun yet.
    started: bool,
    /// The session is still alive: leave the lock held.
    keep: bool,
}

impl LockGuard {
    fn acquire(inner: &Arc<Inner>) -> Option<LockGuard> {
        inner.lock.try_acquire().then(|| LockGuard {
            inner: inner.clone(),
            started: true,
            keep: false,
        })
    }

    /// The session is not over: the lock stays held on drop.
    fn keep_held(mut self) {
        self.keep = true;
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        if std::thread::panicking() {
            self.inner
                .block_and_report("the session stopped unexpectedly".into());
        } else if !self.started {
            self.inner.block("the session worker never started".into());
        }
        self.inner.lock.release();
    }
}

/// How the startup cleanup ended.
enum Recovered {
    Cleaned(usize),
    StillRunning(GameInfo),
}

/// What `prepare_and_launch` achieved.
enum Step {
    /// Deployed and opened: track the new session.
    Launched(GameInfo),
    /// The game of an unfinished session is still running: resume tracking.
    Resume(GameInfo),
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
            status: Mutex::new(RecoveryStatus {
                state: RecoveryState::NotRun,
                error: None,
                cleaned_count: 0,
            }),
        }))
    }

    /// Cleans a session left deployed by a crash (same ownership and
    /// authorized-directory rules as any cleanup), unless its game is still
    /// running: then the files stay and tracking resumes until it ends. Must
    /// run before the first `play`. A failure is reported, keeps the pending
    /// items and blocks new deploys until a later cleanup succeeds.
    pub fn recover_on_startup(&self) -> Result<usize, PlayError> {
        let inner = &self.0;
        let guard = LockGuard::acquire(inner).ok_or(PlayError::AlreadyRunning)?;
        match inner.try_recover() {
            Ok(Recovered::Cleaned(cleaned)) => {
                drop(guard);
                *relock(&inner.recovery) = Recovery::Ready;
                inner.set_status(RecoveryState::Recovered, None, cleaned);
                Ok(cleaned)
            }
            Ok(Recovered::StillRunning(game)) => {
                inner.mark_game_running();
                match inner.start_worker(game, guard) {
                    Ok(()) => Ok(0),
                    Err(error) => {
                        inner.fail_startup(&error);
                        Err(error)
                    }
                }
            }
            Err(error) => {
                drop(guard);
                inner.fail_startup(&error);
                Err(error)
            }
        }
    }

    /// Files removed by the startup recovery; `None` until it has run.
    pub fn recovered_count(&self) -> Option<usize> {
        let status = relock(&self.0.status);
        (status.state == RecoveryState::Recovered).then_some(status.cleaned_count)
    }

    /// The recovery state for the interface, readable after it subscribed to
    /// the events (a startup failure stays here).
    pub fn recovery_status(&self) -> RecoveryStatus {
        relock(&self.0.status).clone()
    }

    /// Read-only: how the manifest relates to the configured game install.
    pub fn deploy_session(&self) -> Result<DeploySession, PlayError> {
        let game = self.0.game.configured_game()?;
        Ok(self.0.deployer.session(&game.mods_dir)?)
    }

    /// Recovery: drops a manifest of a previous game install without touching
    /// any file. Refused while a game or session is active.
    pub fn forget_stale_session(&self) -> Result<(), PlayError> {
        self.recover_stale(|deployer, mods_dir| deployer.forget_stale_session(mods_dir).map(|()| 0))
            .map(|_| ())
    }

    /// Recovery: cleans the folder of a previous game install, only if
    /// `confirmed` is exactly the folder the manifest records. Returns the
    /// number of files removed. Refused while a game or session is active.
    pub fn clean_stale_session(&self, confirmed: &Path) -> Result<usize, PlayError> {
        self.recover_stale(|deployer, mods_dir| deployer.clean_stale_session(mods_dir, confirmed))
    }

    fn recover_stale(
        &self,
        action: impl FnOnce(&dyn Deployer, &Path) -> Result<usize, DeployError>,
    ) -> Result<usize, PlayError> {
        let inner = &self.0;
        if matches!(*relock(&inner.recovery), Recovery::GameRunning) {
            return Err(PlayError::GameStillRunning);
        }
        let _guard = LockGuard::acquire(inner).ok_or(PlayError::AlreadyRunning)?;
        let game = inner.game.configured_game()?;
        let cleaned = action(inner.deployer.as_ref(), &game.mods_dir)?;
        // The manifest is empty now: nothing is pending, so Play is allowed.
        let mut recovery = relock(&inner.recovery);
        if matches!(*recovery, Recovery::Blocked) {
            *recovery = Recovery::Ready;
            drop(recovery);
            inner.set_status(RecoveryState::Recovered, None, cleaned);
        }
        Ok(cleaned)
    }

    /// Validates, deploys, opens the game and tracks the session in the
    /// background. The lock is held from here until the cleanup finishes.
    pub fn play(&self, profile_id: &str) -> Result<(), PlayError> {
        let inner = &self.0;
        if matches!(*relock(&inner.recovery), Recovery::GameRunning) {
            return Err(PlayError::GameStillRunning);
        }
        let guard = LockGuard::acquire(inner).ok_or(PlayError::AlreadyRunning)?;
        match inner.prepare_and_launch(profile_id)? {
            Step::Launched(game) => inner.start_worker(game, guard),
            Step::Resume(game) => {
                inner.mark_game_running();
                inner.start_worker(game, guard)?;
                Err(PlayError::GameStillRunning)
            }
        }
    }
}

fn relock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn inspection(error: AppError) -> PlayError {
    PlayError::ProcessInspection {
        detail: error.to_string(),
    }
}

impl Inner {
    fn try_recover(&self) -> Result<Recovered, PlayError> {
        if self.deployer.is_clean()? {
            return Ok(Recovered::Cleaned(0));
        }
        let game = self.game.configured_game()?;
        // Never delete what a running game may be using.
        if self.runtime.game_running(&game.root).map_err(inspection)? {
            return Ok(Recovered::StillRunning(game));
        }
        Ok(Recovered::Cleaned(self.deployer.clean(&game.mods_dir)?))
    }

    fn set_status(&self, state: RecoveryState, error: Option<String>, cleaned_count: usize) {
        *relock(&self.status) = RecoveryStatus {
            state,
            error,
            cleaned_count,
        };
    }

    fn mark_game_running(&self) {
        *relock(&self.recovery) = Recovery::GameRunning;
        self.set_status(RecoveryState::GameRunning, None, 0);
    }

    fn fail_startup(&self, error: &PlayError) {
        let message = error.to_string();
        self.set_status(RecoveryState::Failed, Some(message.clone()), 0);
        self.block_and_report(message);
    }

    /// New deploys wait until a later cleanup succeeds.
    fn block(&self, message: String) {
        *relock(&self.recovery) = Recovery::Blocked;
        let mut status = relock(&self.status);
        if status.state == RecoveryState::GameRunning {
            *status = RecoveryStatus {
                state: RecoveryState::Failed,
                error: Some(message),
                cleaned_count: 0,
            };
        }
    }

    fn block_and_report(&self, message: String) {
        self.block(message.clone());
        self.publisher.report_error(&message);
    }

    /// Hands the session (and its lock) to a background worker. When the
    /// worker cannot start, the lock is released with the cleanup pending.
    fn start_worker(
        self: &Arc<Self>,
        game: GameInfo,
        mut guard: LockGuard,
    ) -> Result<(), PlayError> {
        guard.started = false;
        let shared = self.clone();
        let task = move || {
            let mut guard = guard;
            guard.started = true;
            shared.run_session(&game, guard);
        };
        self.runner
            .spawn(Box::new(task))
            .map_err(|error| PlayError::SessionThread { detail: error.0 })
    }

    /// Everything up to (and including) opening the game. On `Err` nothing of
    /// this attempt is left deployed unless the compensation itself failed,
    /// in which case new deploys are blocked.
    fn prepare_and_launch(&self, profile_id: &str) -> Result<Step, PlayError> {
        let blocked = match &*relock(&self.recovery) {
            Recovery::NotRun => {
                return Err(PlayError::RecoveryPending {
                    detail: "startup recovery has not run".into(),
                })
            }
            Recovery::Ready => false,
            Recovery::Blocked => true,
            Recovery::GameRunning => return Err(PlayError::GameStillRunning),
        };
        // Validate the prerequisites before touching ~mods.
        let game = self.game.configured_game()?;
        launch_url(&game)?;
        // Never deploy over, or clean under, a game that is still running.
        if self.runtime.game_running(&game.root).map_err(inspection)? {
            return if blocked {
                Ok(Step::Resume(game))
            } else {
                Err(PlayError::GameStillRunning)
            };
        }
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
        Ok(Step::Launched(game))
    }

    fn retry_blocked_cleanup(&self, game: &GameInfo) -> Result<(), PlayError> {
        match self.deployer.clean(&game.mods_dir) {
            Ok(cleaned) => {
                *relock(&self.recovery) = Recovery::Ready;
                self.set_status(RecoveryState::Recovered, None, cleaned);
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
            self.block_and_report(error.to_string());
        }
    }

    /// Background part of a session: publish `started`, then clean and
    /// publish the final event. The guard releases the lock when it is done
    /// (or when this unwinds).
    fn run_session(&self, game: &GameInfo, guard: LockGuard) {
        let mut terminal = None;
        let outcome = self.runtime.track(&game.root, &mut |event| match event {
            WatchEvent::Started => self.publisher.publish(SessionEvent {
                state: SessionState::Started,
                cleaned: 0,
            }),
            other => terminal = Some(other),
        });
        match (outcome, terminal) {
            (Ok(TrackOutcome::Finished), Some(event)) => self.finish(game, event, guard),
            (Err(error), _) => self.tracking_failed(&error, guard),
            // Cancelled (or no terminal event): the session is not over, so
            // nothing is cleaned and the lock stays held.
            _ => guard.keep_held(),
        }
    }

    fn finish(&self, game: &GameInfo, event: WatchEvent, guard: LockGuard) {
        let state = match event {
            WatchEvent::Started => return guard.keep_held(),
            WatchEvent::Ended => SessionState::Ended,
            WatchEvent::NeverStarted => SessionState::NeverStarted,
        };
        match self.deployer.clean(&game.mods_dir) {
            Ok(cleaned) => {
                *relock(&self.recovery) = Recovery::Ready;
                let resumed = relock(&self.status).state == RecoveryState::GameRunning;
                if resumed {
                    self.set_status(RecoveryState::Recovered, None, cleaned);
                }
                drop(guard);
                self.publisher.publish(SessionEvent { state, cleaned });
            }
            Err(error) => {
                self.block_and_report(error.to_string());
                drop(guard);
            }
        }
    }

    /// The watcher died: the files stay deployed (the game may still be
    /// running) and the next Play checks the game before cleaning.
    fn tracking_failed(&self, error: &AppError, guard: LockGuard) {
        self.block_and_report(format!("could not follow the game session: {error}"));
        drop(guard);
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

    fn session(&self, authorized_mods_dir: &Path) -> Result<DeploySession, DeployError> {
        DeployService::session(self, authorized_mods_dir)
    }

    fn forget_stale_session(&self, authorized_mods_dir: &Path) -> Result<(), DeployError> {
        DeployService::forget_stale_session(self, authorized_mods_dir)
    }

    fn clean_stale_session(
        &self,
        authorized_mods_dir: &Path,
        confirmed: &Path,
    ) -> Result<usize, DeployError> {
        DeployService::clean_stale_session(self, authorized_mods_dir, confirmed)
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

    fn game_running(&self, root: &Path) -> Result<bool, AppError> {
        SessionService::game_running(self, root)
    }

    fn track(
        &self,
        root: &Path,
        emit: &mut dyn FnMut(WatchEvent),
    ) -> Result<TrackOutcome, AppError> {
        SessionService::track(self, root, || false, emit)
    }
}
