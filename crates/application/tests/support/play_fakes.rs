//! Thread-safe fakes for the Play orchestrator (issue #8). The deploy side
//! uses the real `DeployService` over the in-memory `deploy_fakes` ports, so
//! ownership and authorized-directory rules are exercised for real.
#![allow(dead_code)]
#[path = "deploy_fakes.rs"]
pub mod deploy;

use deploy::{mod_of, mods_dir, put_sources, service, Shared, ID_A};
use motogp_application::{
    AppError, BackgroundRunner, DeployMod, Deployer, ModPlanner, PlayError, PlayGame, PlayService,
    SessionError, SessionEvent, SessionLock, SessionPublisher, SessionRuntime, TrackOutcome,
};
use motogp_domain::{GameInfo, WatchEvent};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
}

impl ModPlanner for FakePlanner {
    fn plan(&self, profile_id: &str) -> Result<Vec<DeployMod>, PlayError> {
        self.requested.lock().unwrap().push(profile_id.to_string());
        Ok(self.mods.lock().unwrap().clone())
    }
}

pub struct FakeRuntime {
    pub launch_result: Mutex<Result<(), SessionError>>,
    pub launches: AtomicUsize,
    pub events: Mutex<Vec<WatchEvent>>,
    pub outcome: Mutex<Result<TrackOutcome, AppError>>,
}

impl SessionRuntime for FakeRuntime {
    fn launch(&self, _game: &GameInfo) -> Result<(), SessionError> {
        self.launches.fetch_add(1, Ordering::SeqCst);
        self.launch_result.lock().unwrap().clone()
    }

    fn track(
        &self,
        _root: &Path,
        emit: &mut dyn FnMut(WatchEvent),
    ) -> Result<TrackOutcome, AppError> {
        for event in self.events.lock().unwrap().iter() {
            emit(*event);
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
pub struct ManualRunner(Mutex<Vec<Task>>);

impl BackgroundRunner for ManualRunner {
    fn spawn(&self, task: Task) {
        self.0.lock().unwrap().push(task);
    }
}

impl ManualRunner {
    pub fn run_pending(&self) {
        let tasks: Vec<Task> = self.0.lock().unwrap().drain(..).collect();
        for task in tasks {
            task();
        }
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
        });
        let runtime = Arc::new(FakeRuntime {
            launch_result: Mutex::new(Ok(())),
            launches: AtomicUsize::new(0),
            events: Mutex::new(vec![WatchEvent::Started, WatchEvent::Ended]),
            outcome: Mutex::new(Ok(TrackOutcome::Finished)),
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
