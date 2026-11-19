//! Process inspection adapter (`sysinfo`) and the pure path rules it relies on.
use motogp_application::{AppError, ProcessInspector};
use std::path::{Path, PathBuf};

/// True when `exe` is `root` itself or lives under it. Components are compared
/// whole and ignoring case, so `MotoGP25Backup` is not inside `MotoGP25`.
pub fn is_inside_root(exe: &Path, root: &Path) -> bool {
    let _ = (exe, root);
    todo!()
}

/// True when any known executable path is inside `root`. A process whose
/// executable could not be read (`None`) is ignored: it proves nothing.
pub fn any_inside_root<I>(executables: I, root: &Path) -> bool
where
    I: IntoIterator<Item = Option<PathBuf>>,
{
    let _ = (executables, root);
    todo!()
}

/// `ProcessInspector` backed by the real process table.
#[derive(Debug, Default, Clone, Copy)]
pub struct SysinfoProcesses;

impl ProcessInspector for SysinfoProcesses {
    fn game_running(&self, root: &Path) -> Result<bool, AppError> {
        let _ = root;
        todo!()
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
    fn real_inspector_does_not_find_a_game_in_an_unused_root() {
        let root = Path::new("Z:/definitely/not/a/real/game/root");
        assert_eq!(SysinfoProcesses.game_running(root), Ok(false));
    }
}
