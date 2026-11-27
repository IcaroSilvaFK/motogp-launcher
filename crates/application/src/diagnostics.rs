//! Diagnostics for bug reports (issue #148): the log file, the OS and the
//! recent log lines, with anything secret-looking removed.

use crate::{AppError, FolderOpener};
use std::path::PathBuf;

/// How many log lines a diagnostics report carries.
pub const DIAGNOSTICS_LOG_LINES: usize = 50;

/// What replaces a removed secret.
pub const REDACTED: &str = "[redacted]";

/// Port: the launcher log files on disk.
pub trait LogFiles {
    /// Folder that holds the log files.
    fn dir(&self) -> PathBuf;

    /// The last `max` log lines, oldest first. No log yet is an empty list.
    fn recent_lines(&self, max: usize) -> Result<Vec<String>, AppError>;
}

impl<T: LogFiles + ?Sized> LogFiles for std::sync::Arc<T> {
    fn dir(&self) -> PathBuf {
        (**self).dir()
    }

    fn recent_lines(&self, max: usize) -> Result<Vec<String>, AppError> {
        (**self).recent_lines(max)
    }
}

/// Port: facts about the machine.
pub trait SystemInfo {
    /// Human-readable OS name and version.
    fn os_version(&self) -> String;
}

/// What the backend knows for a diagnostics report. The frontend adds the
/// facts it owns (launcher version, game folder, mods, active profile).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsSnapshot {
    pub os: String,
    pub log_lines: Vec<String>,
}

/// Use case: collects diagnostics and opens the logs folder.
pub struct DiagnosticsService<L, S, O> {
    logs: L,
    system: S,
    opener: O,
}

impl<L: LogFiles, S: SystemInfo, O: FolderOpener> DiagnosticsService<L, S, O> {
    pub fn new(logs: L, system: S, opener: O) -> Self {
        Self {
            logs,
            system,
            opener,
        }
    }

    /// The OS and the recent log lines, redacted again on the way out so a
    /// secret that reached the file by another route never leaves the app.
    /// An unreadable log is an empty list: a report is still useful without it.
    pub fn snapshot(&self) -> DiagnosticsSnapshot {
        let log_lines = self
            .logs
            .recent_lines(DIAGNOSTICS_LOG_LINES)
            .unwrap_or_default()
            .iter()
            .map(|line| redact_secrets(line))
            .collect();
        DiagnosticsSnapshot {
            os: self.system.os_version(),
            log_lines,
        }
    }

    /// Opens exactly the logs folder; the caller never chooses the path.
    pub fn open_logs_folder(&self) -> Result<(), AppError> {
        self.opener.open_folder(&self.logs.dir())
    }
}

/// Words that announce a secret value right after them.
const SECRET_KEYS: [&str; 6] = [
    "token",
    "authorization",
    "bearer",
    "password",
    "secret",
    "cookie",
];

/// Shortest run that is treated as an opaque credential on its own.
const LONG_TOKEN: usize = 24;

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+' | '=')
}

/// Removes anything that looks like a credential: the value after a key such
/// as `token=`, `Authorization:` or `Bearer `, and any long opaque run that
/// mixes letters and digits.
pub fn redact_secrets(line: &str) -> String {
    redact_long_tokens(&redact_keyed_values(line))
}

fn redact_keyed_values(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let mut out = String::with_capacity(line.len());
    let mut at = 0;
    while at < line.len() {
        let hit = SECRET_KEYS
            .iter()
            .filter_map(|key| lower[at..].find(key).map(|found| (at + found, key.len())))
            .min_by_key(|(start, _)| *start);
        let Some((start, len)) = hit else {
            out.push_str(&line[at..]);
            break;
        };
        let mut end = start + len;
        out.push_str(&line[at..end]);
        // Skip the separator between the key and its value.
        let rest = &line[end..];
        let skipped = rest
            .find(|c: char| !matches!(c, '=' | ':' | ' ' | '"' | '\''))
            .unwrap_or(rest.len());
        let value_start = end + skipped;
        let value = &line[value_start..];
        let mut value_len = word_len(value);
        // "Bearer <credential>": the scheme word is not the secret.
        if value[..value_len].eq_ignore_ascii_case("bearer")
            || value[..value_len].eq_ignore_ascii_case("basic")
        {
            let after = &value[value_len..];
            let gap = after.len() - after.trim_start().len();
            value_len += gap + word_len(&after[gap..]);
        }
        if value_len > 0 {
            out.push_str(&line[end..value_start]);
            out.push_str(REDACTED);
            end = value_start + value_len;
        }
        at = end;
    }
    out
}

fn word_len(text: &str) -> usize {
    text.find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';' | '&'))
        .unwrap_or(text.len())
}

fn redact_long_tokens(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        let long = run.chars().count() >= LONG_TOKEN
            && run.chars().any(|c| c.is_ascii_digit())
            && run.chars().any(|c| c.is_ascii_alphabetic());
        out.push_str(if long { REDACTED } else { run.as_str() });
        run.clear();
    };
    for c in line.chars() {
        if is_token_char(c) {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::path::Path;

    struct Logs {
        lines: Result<Vec<String>, AppError>,
        asked: RefCell<Vec<usize>>,
    }

    impl Logs {
        fn with(lines: &[&str]) -> Self {
            Self {
                lines: Ok(lines.iter().map(|l| l.to_string()).collect()),
                asked: RefCell::default(),
            }
        }
    }

    impl LogFiles for &Logs {
        fn dir(&self) -> PathBuf {
            PathBuf::from("C:/data/logs")
        }
        fn recent_lines(&self, max: usize) -> Result<Vec<String>, AppError> {
            self.asked.borrow_mut().push(max);
            self.lines.clone()
        }
    }

    struct Os;
    impl SystemInfo for Os {
        fn os_version(&self) -> String {
            "Windows 11".into()
        }
    }

    #[derive(Default)]
    struct Opener {
        opened: RefCell<Vec<PathBuf>>,
    }
    impl FolderOpener for &Opener {
        fn open_folder(&self, path: &Path) -> Result<(), AppError> {
            self.opened.borrow_mut().push(path.to_path_buf());
            Ok(())
        }
    }

    #[test]
    fn snapshot_has_the_os_and_asks_for_fifty_lines() {
        let logs = Logs::with(&["a", "b"]);
        let opener = Opener::default();
        let snapshot = DiagnosticsService::new(&logs, Os, &opener).snapshot();
        assert_eq!(snapshot.os, "Windows 11");
        assert_eq!(snapshot.log_lines, vec!["a", "b"]);
        assert_eq!(*logs.asked.borrow(), vec![50]);
    }

    #[test]
    fn snapshot_redacts_log_lines() {
        let logs = Logs::with(&["INFO sign-in ok token=abc123 done"]);
        let opener = Opener::default();
        let snapshot = DiagnosticsService::new(&logs, Os, &opener).snapshot();
        assert_eq!(
            snapshot.log_lines,
            vec!["INFO sign-in ok token=[redacted] done"]
        );
    }

    #[test]
    fn unreadable_log_is_an_empty_list() {
        let mut logs = Logs::with(&[]);
        logs.lines = Err(AppError::Read {
            location: "x".into(),
            detail: "denied".into(),
        });
        let opener = Opener::default();
        let snapshot = DiagnosticsService::new(&logs, Os, &opener).snapshot();
        assert!(snapshot.log_lines.is_empty());
    }

    #[test]
    fn opens_exactly_the_logs_folder() {
        let logs = Logs::with(&[]);
        let opener = Opener::default();
        DiagnosticsService::new(&logs, Os, &opener)
            .open_logs_folder()
            .unwrap();
        assert_eq!(*opener.opened.borrow(), vec![PathBuf::from("C:/data/logs")]);
    }

    #[test]
    fn redacts_keyed_values() {
        assert_eq!(
            redact_secrets("Authorization: Bearer eyJhbGciOi.payload.sig"),
            "Authorization: [redacted]"
        );
        assert_eq!(
            redact_secrets("cookie=\"sid-1\"; path=/"),
            "cookie=\"[redacted]\"; path=/"
        );
        assert_eq!(
            redact_secrets("{\"sessionToken\":\"s3cr3t\"}"),
            "{\"sessionToken\":\"[redacted]\"}"
        );
    }

    #[test]
    fn redacts_long_opaque_runs_but_keeps_ordinary_text() {
        let secret = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b";
        assert_eq!(
            redact_secrets(&format!("id {secret} end")),
            "id [redacted] end"
        );
        let plain = "Loaded C:/Users/Usuário Padrão/AppData/library/catalog-cache.json";
        assert_eq!(redact_secrets(plain), plain);
    }
}
