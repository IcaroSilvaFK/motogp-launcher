//! Process inspection adapter (`sysinfo`) and the pure path rules it relies on.
use motogp_application::{AppError, ProcessInspector};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::Mutex;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

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
}

#[cfg(test)]
mod tests {
    use super::*;

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
