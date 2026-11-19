//! Process inspection adapter (`sysinfo`) and the pure path rules it relies on.
use motogp_application::{AppError, ProcessInspector};
use std::path::{Path, PathBuf};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// Components lowercased, so `C:/Jogos` and `c:\jogos` compare equal.
fn normalized(path: &Path) -> Vec<String> {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

/// True when `exe` is `root` itself or lives under it. Components are compared
/// whole and ignoring case, so `MotoGP25Backup` is not inside `MotoGP25`.
pub fn is_inside_root(exe: &Path, root: &Path) -> bool {
    let root = normalized(root);
    !root.is_empty() && normalized(exe).starts_with(&root)
}

/// True when any known executable path is inside `root`. A process whose
/// executable could not be read (`None`) is ignored: it proves nothing.
pub fn any_inside_root<I>(executables: I, root: &Path) -> bool
where
    I: IntoIterator<Item = Option<PathBuf>>,
{
    executables
        .into_iter()
        .flatten()
        .any(|exe| is_inside_root(&exe, root))
}

/// `ProcessInspector` backed by the real process table.
#[derive(Debug, Default, Clone, Copy)]
pub struct SysinfoProcesses;

impl ProcessInspector for SysinfoProcesses {
    fn game_running(&self, root: &Path) -> Result<bool, AppError> {
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        let executables = system
            .processes()
            .values()
            .map(|process| process.exe().map(Path::to_path_buf));
        Ok(any_inside_root(executables, root))
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
        assert!(is_inside_root(Path::new("C:/Jogos/MotoGP25/bin/../game.exe"), root));
        assert!(!is_inside_root(Path::new("C:/Jogos/MotoGP25/../Other/a.exe"), root));
        assert!(is_inside_root(Path::new("C:/Jogos/Other/../MotoGP25/a.exe"), root));
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_prefix_matches_the_plain_path() {
        let root = Path::new("C:/Jogos/MotoGP25");
        assert!(is_inside_root(Path::new(r"\?C:JogosMotoGP25game.exe"), root));
        assert!(is_inside_root(Path::new("C:/Jogos/MotoGP25/game.exe"), Path::new(r"\?C:JogosMotoGP25")));
        assert!(!is_inside_root(Path::new(r"\?C:JogosMotoGP25Backup.exe"), root));
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
        assert_eq!(SysinfoProcesses.game_running(root), Ok(false));
    }
}
