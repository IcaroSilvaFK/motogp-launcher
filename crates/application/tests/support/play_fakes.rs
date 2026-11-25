//! Thread-safe fakes for the Play orchestrator (issue #8). The deploy side
//! uses the real `DeployService` over the in-memory `deploy_fakes` ports, so
//! ownership and authorized-directory rules are exercised for real.
#![allow(dead_code)]
pub use crate::deploy_fakes as deploy;

use deploy::{mod_of, mods_dir, put_sources, service, Shared, ID_A};
use motogp_application::{
    AppError, BackgroundRunner, DeployMod, Deployer, ModPlanner, PlayError, PlayGame, PlayService,
    SessionError, SessionEvent, SessionLock, SessionPublisher, SessionRuntime, SpawnError,
    TrackOutcome,
};
use motogp_domain::{GameInfo, WatchEvent};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub const DEPLOYED_A: &str = "000_aaaaaaaaaaaa_A.pak";

pub fn game(app_id: Option<u32>) -> GameInfo {
    let mut game = GameInfo::new(PathBuf::from("game"), "Project", app_id);
    game.mods_dir = mods_dir();
    game
}

pub struct FakeGame(pub Mutex<Result<GameInfo, PlayError>>);

impl PlayGame for FakeGame {
    fn configured_game(&self) -> Result<GameInfo, PlayError> {
        self.0.lock().unwrap().clone()
    }
}

pub struct FakePlanner {
    pub mods: Mutex<Vec<DeployMod>>,
    pub requested: Mutex<Vec<String>>,
    pub panics: AtomicBool,
}

impl ModPlanner for FakePlanner {
    fn plan(&self, profile_id: &str) -> Result<Vec<DeployMod>, PlayError> {
        if self.panics.load(Ordering::SeqCst) {
            panic!("planner exploded");
        }
        self.requested.lock().unwrap().push(profile_id.to_string());
        Ok(self.mods.lock().unwrap().clone())
    }
}

pub struct FakeRuntime {
    pub launch_result: Mutex<Result<(), SessionError>>,
    pub launches: AtomicUsize,
    pub events: Mutex<Vec<WatchEvent>>,
    pub outcome: Mutex<Result<TrackOutcome, AppError>>,
    /// What the process inspection answers (`game_running`).
    pub running: Mutex<Result<bool, AppError>>,
    pub track_panics: AtomicBool,
    /// Keep tracking after the scripted events until a stop is requested
    /// (the worker then ends the session), like a live game.
    pub hold: AtomicBool,
    /// The held game closes by itself (without a stop request).
    pub exit: AtomicBool,
    pub terminate_result: Mutex<Result<usize, AppError>>,
    pub terminations: AtomicUsize,
    pub now_ms: AtomicU64,
}

impl SessionRuntime for FakeRuntime {
    fn launch(&self, _game: &GameInfo) -> Result<(), SessionError> {
        self.launches.fetch_add(1, Ordering::SeqCst);
        self.launch_result.lock().unwrap().clone()
    }

    fn game_running(&self, _root: &Path) -> Result<bool, AppError> {
        self.running.lock().unwrap().clone()
    }

    fn terminate(&self, _root: &Path) -> Result<usize, AppError> {
        self.terminations.fetch_add(1, Ordering::SeqCst);
        self.terminate_result.lock().unwrap().clone()
    }

    fn unix_now_ms(&self) -> u64 {
        self.now_ms.load(Ordering::SeqCst)
    }

    fn track(
        &self,
        _root: &Path,
        stop_requested: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(WatchEvent),
    ) -> Result<TrackOutcome, AppError> {
        if self.track_panics.load(Ordering::SeqCst) {
            panic!("tracking exploded");
        }
        for event in self.events.lock().unwrap().iter() {
            emit(*event);
        }
        if self.hold.load(Ordering::SeqCst) {
            // A live game: the session only ends once a stop is requested.
            let give_up = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !stop_requested() && !self.exit.load(Ordering::SeqCst) {
                assert!(std::time::Instant::now() < give_up, "stop never requested");
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            emit(WatchEvent::Ended);
            return Ok(TrackOutcome::Finished);
        }
        self.outcome.lock().unwrap().clone()
    }
}

#[derive(Default)]
pub struct AtomicLock(AtomicBool);

impl SessionLock for AtomicLock {
    fn try_acquire(&self) -> bool {
        !self.0.swap(true, Ordering::SeqCst)
    }
    fn release(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

type Task = Box<dyn FnOnce() + Send + 'static>;

/// Queues the background work so tests decide when the session "runs".
#[derive(Default)]
pub struct ManualRunner {
    tasks: Mutex<Vec<Task>>,
    /// When set, `spawn` fails and drops the task (like a failed thread).
    pub fail_spawn: AtomicBool,
    /// When set, `spawn` runs the task on a real thread right away.
    pub threaded: AtomicBool,
}

impl BackgroundRunner for ManualRunner {
    fn spawn(&self, task: Task) -> Result<(), SpawnError> {
        if self.fail_spawn.load(Ordering::SeqCst) {
            return Err(SpawnError("no threads left".into()));
        }
        if self.threaded.load(Ordering::SeqCst) {
            std::thread::spawn(task);
            return Ok(());
        }
        self.tasks.lock().unwrap().push(task);
        Ok(())
    }
}

impl ManualRunner {
    pub fn run_pending(&self) {
        let tasks: Vec<Task> = self.tasks.lock().unwrap().drain(..).collect();
        for task in tasks {
            task();
        }
    }

    pub fn pending(&self) -> usize {
        self.tasks.lock().unwrap().len()
    }
}

#[derive(Default)]
pub struct Recorder {
    pub events: Mutex<Vec<SessionEvent>>,
    pub errors: Mutex<Vec<String>>,
}

impl SessionPublisher for Recorder {
    fn publish(&self, event: SessionEvent) {
        self.events.lock().unwrap().push(event);
    }
    fn report_error(&self, message: &str) {
        self.errors.lock().unwrap().push(message.to_string());
    }
}

pub struct Rig {
    pub disk: Shared,
    pub game: Arc<FakeGame>,
    pub planner: Arc<FakePlanner>,
    pub runtime: Arc<FakeRuntime>,
    pub lock: Arc<AtomicLock>,
    pub runner: Arc<ManualRunner>,
    pub recorder: Arc<Recorder>,
    pub play: PlayService,
}

#[allow(clippy::new_without_default)]
impl Rig {
    /// A configured game with an App ID, one mod `A.pak` in the profile and a
    /// session that starts and then ends. Startup recovery has already run.
    pub fn ready() -> Rig {
        let rig = Rig::new();
        rig.play.recover_on_startup().expect("startup recovery");
        rig
    }

    /// Like `ready` but startup recovery has not run yet.
    pub fn new() -> Rig {
        let disk = deploy::new_state();
        let mods = vec![mod_of(ID_A, &["A.pak"])];
        put_sources(&disk, &mods);
        let game = Arc::new(FakeGame(Mutex::new(Ok(self::game(Some(1))))));
        let planner = Arc::new(FakePlanner {
            mods: Mutex::new(mods),
            requested: Mutex::default(),
            panics: AtomicBool::new(false),
        });
        let runtime = Arc::new(FakeRuntime {
            launch_result: Mutex::new(Ok(())),
            launches: AtomicUsize::new(0),
            events: Mutex::new(vec![WatchEvent::Started, WatchEvent::Ended]),
            outcome: Mutex::new(Ok(TrackOutcome::Finished)),
            running: Mutex::new(Ok(false)),
            track_panics: AtomicBool::new(false),
            hold: AtomicBool::new(false),
            exit: AtomicBool::new(false),
            terminate_result: Mutex::new(Ok(1)),
            terminations: AtomicUsize::new(0),
            now_ms: AtomicU64::new(1_700_000_000_000),
        });
        let lock = Arc::new(AtomicLock::default());
        let runner = Arc::new(ManualRunner::default());
        let recorder = Arc::new(Recorder::default());
        let deployer: Arc<dyn Deployer> = Arc::new(service(&disk));
        let play = PlayService::new(
            game.clone(),
            planner.clone(),
            deployer,
            runtime.clone(),
            lock.clone(),
            runner.clone(),
            recorder.clone(),
        );
        Rig {
            disk,
            game,
            planner,
            runtime,
            lock,
            runner,
            recorder,
            play,
        }
    }

    /// Like `ready`, but the session runs on a real thread, the game "stays
    /// open" until a stop is requested, and the profile deploys `files`.
    pub fn live(files: &[&str]) -> Rig {
        let rig = Rig::new();
        let mods = vec![mod_of(ID_A, files)];
        put_sources(&rig.disk, &mods);
        *rig.planner.mods.lock().unwrap() = mods;
        *rig.runtime.events.lock().unwrap() = vec![WatchEvent::Started];
        rig.runtime.hold.store(true, Ordering::SeqCst);
        rig.runner.threaded.store(true, Ordering::SeqCst);
        rig.play.recover_on_startup().expect("startup recovery");
        rig
    }

    /// Waits (bounded) for `condition`, polling the live session.
    pub fn wait_until(&self, what: &str, condition: impl Fn(&Rig) -> bool) {
        let give_up = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !condition(self) {
            assert!(std::time::Instant::now() < give_up, "timed out: {what}");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    pub fn set_game(&self, game: Result<GameInfo, PlayError>) {
        *self.game.0.lock().unwrap() = game;
    }

    pub fn events(&self) -> Vec<SessionEvent> {
        self.recorder.events.lock().unwrap().clone()
    }

    pub fn errors(&self) -> Vec<String> {
        self.recorder.errors.lock().unwrap().clone()
    }

    pub fn launches(&self) -> usize {
        self.runtime.launches.load(Ordering::SeqCst)
    }

    pub fn materialized(&self) -> usize {
        self.disk.borrow().materialize_calls
    }
}

impl Drop for Rig {
    /// Lets a held game of a finished test close, so its thread ends quietly.
    fn drop(&mut self) {
        self.runtime.exit.store(true, Ordering::SeqCst);
    }
}
