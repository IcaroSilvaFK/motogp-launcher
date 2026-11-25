//! Process inspection adapter (`sysinfo`) and the pure path rules it relies on.
use motogp_application::{AppError, ProcessInspector};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::Mutex;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Lexical normalization: case-insensitive components, `.` dropped, `..`
/// applied, and verbatim prefixes (`\?\C:`, `\?\UNC\s\sh`) mapped to their
/// plain form so both spellings compare equal.
fn normalized(path: &Path) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.last().is_some_and(|last| !is_anchor(last)) {
                    parts.pop();
                }
            }
            Component::Prefix(prefix) => parts.push(match prefix.kind() {
                Prefix::Disk(d) | Prefix::VerbatimDisk(d) => {
                    format!("{}:", char::from(d).to_ascii_lowercase())
                }
                Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => format!(
                    r"\\{}\{}",
                    server.to_string_lossy().to_lowercase(),
                    share.to_string_lossy().to_lowercase()
                ),
                _ => prefix.as_os_str().to_string_lossy().to_lowercase(),
            }),
            Component::RootDir => parts.push(std::path::MAIN_SEPARATOR.to_string()),
            Component::Normal(name) => parts.push(name.to_string_lossy().to_lowercase()),
        }
    }
    parts
}

fn is_anchor(part: &str) -> bool {
    part == std::path::MAIN_SEPARATOR.to_string() || part.ends_with(':') || part.starts_with("\\\\")
}

/// Resolves junctions, symlinks and 8.3 short names with the file system when
/// the path is reachable; otherwise (process gone, access denied, fake path)
/// falls back to the lexical form of the path as given.
fn resolved(path: &Path) -> Vec<String> {
    match std::fs::canonicalize(path) {
        Ok(real) => normalized(&real),
        Err(_) => normalized(path),
    }
}

fn starts_with(exe: &[String], root: &[String]) -> bool {
    !root.is_empty() && exe.starts_with(root)
}

/// True when `exe` is `root` itself or lives under it. Both paths are resolved
/// and compared by whole components, ignoring case, so `MotoGP25Backup` is not
/// inside `MotoGP25`.
pub fn is_inside_root(exe: &Path, root: &Path) -> bool {
    starts_with(&resolved(exe), &resolved(root))
}

/// True when any known executable path is inside `root`. A process whose
/// executable could not be read (`None`) is ignored: it proves nothing.
pub fn any_inside_root<I>(executables: I, root: &Path) -> bool
where
    I: IntoIterator<Item = Option<PathBuf>>,
{
    let root = resolved(root);
    executables
        .into_iter()
        .flatten()
        .any(|exe| starts_with(&resolved(&exe), &root))
}

/// State kept between polls: the process table, the resolved root (computed
/// once per root) and resolved executables of live processes.
#[derive(Default)]
struct Inspection {
    system: System,
    root: Option<(PathBuf, Vec<String>)>,
    exes: HashMap<PathBuf, Vec<String>>,
}

/// `ProcessInspector` backed by the real process table.
#[derive(Default)]
pub struct SysinfoProcesses {
    state: Mutex<Inspection>,
}

impl ProcessInspector for SysinfoProcesses {
    fn game_running(&self, root: &Path) -> Result<bool, AppError> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let Inspection {
            system,
            root: cached_root,
            exes,
        } = &mut *guard;
        if cached_root.as_ref().map(|(raw, _)| raw.as_path()) != Some(root) {
            *cached_root = Some((root.to_path_buf(), resolved(root)));
        }
        let root = &cached_root.as_ref().expect("root just cached").1;
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        let mut live: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut found = false;
        for exe in system.processes().values().filter_map(|p| p.exe()) {
            if live.contains_key(exe) {
                continue;
            }
            let parts = exes.remove(exe).unwrap_or_else(|| resolved(exe));
            found |= starts_with(&parts, root);
            live.insert(exe.to_path_buf(), parts);
        }
        *exes = live;
        Ok(found)
    }

    fn terminate_inside(&self, root: &Path) -> Result<usize, AppError> {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let system = &mut guard.system;
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        let root = resolved(root);
        let own = std::process::id();
        let seeds: Vec<u32> = system
            .processes()
            .iter()
            .filter(|(_, process)| {
                process
                    .exe()
                    .is_some_and(|exe| starts_with(&resolved(exe), &root))
            })
            .map(|(pid, _)| pid.as_u32())
            .collect();
        let parents: HashMap<u32, u32> = system
            .processes()
            .iter()
            .filter_map(|(pid, process)| Some((pid.as_u32(), process.parent()?.as_u32())))
            .collect();
        let mut ended = 0;
        let mut failures = Vec::new();
        for pid in kill_order(&seeds, &parents, own) {
            let pid = Pid::from_u32(pid);
            let Some(process) = system.process(pid) else {
                continue;
            };
            if process.kill() {
                ended += 1;
                continue;
            }
            // It may have exited on its own since the snapshot.
            system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
            if system.process(pid).is_some() {
                failures.push(pid.to_string());
            }
        }
        if failures.is_empty() {
            Ok(ended)
        } else {
            Err(AppError::Write {
                location: "processes".into(),
                detail: format!("could not end process {}", failures.join(", ")),
            })
        }
    }
}

/// Order in which to end the processes of the game: `seeds` (processes of the
/// game by executable) plus every descendant, deepest first, so children go
/// before their parents. `own` is never included. `parents` maps a pid to its
/// parent pid.
fn kill_order(seeds: &[u32], parents: &HashMap<u32, u32>, own: u32) -> Vec<u32> {
    let mut depth: HashMap<u32, usize> = seeds
        .iter()
        .copied()
        .filter(|pid| *pid != own)
        .map(|pid| (pid, 0))
        .collect();
    loop {
        let mut grew = false;
        for (&pid, &parent) in parents {
            if pid == own || depth.contains_key(&pid) {
                continue;
            }
            if let Some(&parent_depth) = depth.get(&parent) {
                depth.insert(pid, parent_depth + 1);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    let mut order: Vec<(u32, usize)> = depth.into_iter().collect();
    order.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    order.into_iter().map(|(pid, _)| pid).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parents(pairs: &[(u32, u32)]) -> HashMap<u32, u32> {
        pairs.iter().copied().collect()
    }

    #[test]
    fn kill_order_ends_descendants_before_their_parents() {
        // 10 is the game, 11 its child, 12 its grandchild; 20 is unrelated.
        let table = parents(&[(11, 10), (12, 11), (20, 1)]);
        assert_eq!(kill_order(&[10], &table, 999), vec![12, 11, 10]);
    }

    #[test]
    fn kill_order_never_includes_the_own_process_or_unrelated_ones() {
        let table = parents(&[(11, 10), (20, 1)]);
        assert_eq!(kill_order(&[10, 999], &table, 999), vec![11, 10]);
        // A child of the own process is not the game's descendant.
        let table = parents(&[(30, 999)]);
        assert!(kill_order(&[], &table, 999).is_empty());
    }

    #[test]
    fn kill_order_survives_a_parent_cycle() {
        let table = parents(&[(1, 2), (2, 1)]);
        assert_eq!(kill_order(&[1], &table, 999), vec![2, 1]);
    }

    #[test]
    fn terminating_in_an_unused_root_ends_nothing() {
        let root = Path::new("Z:/definitely/not/a/real/game/root");
        assert_eq!(SysinfoProcesses::default().terminate_inside(root), Ok(0));
    }

    #[test]
    fn process_path_comparison_is_case_insensitive() {
        assert!(is_inside_root(
            Path::new("c:/jogos/motogp25/Binaries/jogo.exe"),
            Path::new("C:/Jogos/MotoGP25"),
        ));
    }

    #[test]
    fn sibling_directory_is_not_game_root() {
        assert!(!is_inside_root(
            Path::new("C:/Jogos/MotoGP25Backup/outro.exe"),
            Path::new("C:/Jogos/MotoGP25"),
        ));
    }

    #[test]
    fn missing_process_executable_is_ignored() {
        let root = Path::new("C:/Jogos/MotoGP25");
        assert!(!any_inside_root([None, None], root));
        assert!(any_inside_root(
            [None, Some(PathBuf::from("C:/Jogos/MotoGP25/game.exe"))],
            root
        ));
        assert!(!any_inside_root(Vec::new(), root));
    }

    #[test]
    fn root_itself_and_unrelated_paths() {
        let root = Path::new("C:/Jogos/MotoGP25");
        assert!(is_inside_root(root, root));
        assert!(!is_inside_root(Path::new("C:/Jogos"), root));
        assert!(!is_inside_root(Path::new("D:/Jogos/MotoGP25/a.exe"), root));
    }

    #[test]
    fn dot_dot_segments_are_resolved_before_comparing() {
        let root = Path::new("C:/Jogos/MotoGP25");
        assert!(is_inside_root(
            Path::new("C:/Jogos/MotoGP25/bin/../game.exe"),
            root
        ));
        assert!(!is_inside_root(
            Path::new("C:/Jogos/MotoGP25/../Other/a.exe"),
            root
        ));
        assert!(is_inside_root(
            Path::new("C:/Jogos/Other/../MotoGP25/a.exe"),
            root
        ));
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_prefix_matches_the_plain_path() {
        let root = Path::new("C:/Jogos/MotoGP25");
        assert!(is_inside_root(
            Path::new(r"\\?\C:\Jogos\MotoGP25\game.exe"),
            root
        ));
        assert!(is_inside_root(
            Path::new("C:/Jogos/MotoGP25/game.exe"),
            Path::new(r"\\?\C:\Jogos\MotoGP25")
        ));
        assert!(!is_inside_root(
            Path::new(r"\\?\C:\Jogos\MotoGP25Backup\a.exe"),
            root
        ));
    }

    #[cfg(windows)]
    #[test]
    fn executable_under_a_junction_to_the_root_is_inside_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let real = dir.path().join("real");
        let sibling = dir.path().join("real2");
        let link = dir.path().join("link");
        std::fs::create_dir_all(&real).expect("real");
        std::fs::create_dir_all(&sibling).expect("sibling");
        std::fs::write(real.join("game.exe"), b"").expect("exe");
        std::fs::write(sibling.join("game.exe"), b"").expect("exe");
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&real)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !made {
            eprintln!("SKIPPED: could not create a junction with mklink /J");
            return;
        }
        assert!(is_inside_root(&link.join("game.exe"), &real));
        assert!(is_inside_root(&real.join("game.exe"), &link));
        assert!(!is_inside_root(&sibling.join("game.exe"), &real));
        assert!(!is_inside_root(&sibling.join("game.exe"), &link));
    }

    #[test]
    fn real_inspector_reuses_its_state_and_finds_the_current_process() {
        let exe = std::env::current_exe().expect("current exe");
        let root = exe.parent().expect("parent");
        let inspector = SysinfoProcesses::default();
        assert_eq!(inspector.game_running(root), Ok(true));
        assert_eq!(inspector.game_running(root), Ok(true));
    }

    #[test]
    fn real_inspector_does_not_find_a_game_in_an_unused_root() {
        let root = Path::new("Z:/definitely/not/a/real/game/root");
        assert_eq!(SysinfoProcesses::default().game_running(root), Ok(false));
    }
}
