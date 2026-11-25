//! Steps for `tests/features/processes.feature` (issue #7). Controlled paths only.
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::ProcessInspector;
use motogp_infrastructure::{is_inside_root, SysinfoProcesses};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Default, World)]
struct ProcessWorld {
    root: PathBuf,
    executables: Vec<PathBuf>,
    verdicts: Vec<bool>,
    /// Install root and a folder outside it (temp dirs removed on drop).
    install: Option<tempfile::TempDir>,
    elsewhere: Option<tempfile::TempDir>,
    inside: Option<Child>,
    outside: Option<Child>,
    ended: Option<usize>,
}

impl Drop for ProcessWorld {
    /// Never leave a spawned process behind, whatever the scenario did.
    fn drop(&mut self) {
        for child in [&mut self.inside, &mut self.outside].into_iter().flatten() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// A long-running harmless program and its arguments (never touches the game).
fn idle_program() -> (PathBuf, Vec<&'static str>) {
    if cfg!(windows) {
        let system = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        (
            PathBuf::from(system).join("System32").join("PING.EXE"),
            vec!["-n", "60", "127.0.0.1"],
        )
    } else {
        (PathBuf::from("/bin/sleep"), vec!["60"])
    }
}

/// Copies the idle program into `dir` and starts it from there, so its
/// executable path lives inside `dir`.
fn spawn_from(dir: &Path) -> Child {
    let (program, args) = idle_program();
    let copy = dir.join(program.file_name().expect("file name"));
    std::fs::copy(&program, &copy).expect("copy the idle program");
    Command::new(copy)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn the idle program")
}

fn exited_within(child: &mut Child, wait: Duration) -> bool {
    let give_up = Instant::now() + wait;
    while Instant::now() < give_up {
        if child.try_wait().expect("try_wait").is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[given(expr = "a game root {string}")]
fn game_root(w: &mut ProcessWorld, root: String) {
    w.root = PathBuf::from(root);
}

#[given(expr = "executables at {string} and {string}")]
fn executables(w: &mut ProcessWorld, first: String, second: String) {
    w.executables = vec![PathBuf::from(first), PathBuf::from(second)];
}

#[when("I classify the processes")]
fn classify(w: &mut ProcessWorld) {
    w.verdicts = w
        .executables
        .iter()
        .map(|exe| is_inside_root(exe, &w.root))
        .collect();
}

#[then("only the first is considered a game process")]
fn only_first(w: &mut ProcessWorld) {
    assert_eq!(w.verdicts, vec![true, false]);
}

#[given("a process running from inside the install root")]
fn inside_process(w: &mut ProcessWorld) {
    let install = tempfile::tempdir().expect("install root");
    w.inside = Some(spawn_from(install.path()));
    w.install = Some(install);
}

#[given("a process running from outside the install root")]
fn outside_process(w: &mut ProcessWorld) {
    let elsewhere = tempfile::tempdir().expect("other folder");
    w.outside = Some(spawn_from(elsewhere.path()));
    w.elsewhere = Some(elsewhere);
}

#[given("no process runs from inside the install root")]
fn no_process(w: &mut ProcessWorld) {
    w.install = Some(tempfile::tempdir().expect("install root"));
}

#[when("the game processes are terminated")]
fn terminate(w: &mut ProcessWorld) {
    let root = w.install.as_ref().expect("install root").path();
    w.ended = Some(
        SysinfoProcesses::default()
            .terminate_inside(root)
            .expect("terminate"),
    );
}

#[then("the process inside the root has exited")]
fn inside_exited(w: &mut ProcessWorld) {
    let child = w.inside.as_mut().expect("inside");
    assert!(exited_within(child, Duration::from_secs(10)));
    assert_eq!(w.ended, Some(1));
}

#[then("the process outside the root is still running")]
fn outside_running(w: &mut ProcessWorld) {
    let child = w.outside.as_mut().expect("outside");
    assert!(child.try_wait().expect("try_wait").is_none());
}

#[then(expr = "{int} processes were ended")]
fn ended_count(w: &mut ProcessWorld, count: usize) {
    assert_eq!(w.ended, Some(count));
}

/// Runs only `processes.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = ProcessWorld::cucumber()
        .fail_on_skipped()
        .run(crate::feature("processes.feature"))
        .await;
    !writer.execution_has_failed()
}
