//! The launcher log file (issue #148): one small rotating file in the app
//! data folder. Every line is redacted before it is written.

use motogp_application::{redact_secrets, AppError, LogFiles, SystemInfo};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_NAME: &str = "launcher.log";
const OLD_LOG_NAME: &str = "launcher.log.1";
/// The live file rotates past this size, so the folder never holds more than
/// about twice as much.
const MAX_BYTES: u64 = 512 * 1024;

/// Rotating log file under `<dir>/launcher.log`. Writing never fails the
/// caller: a log that cannot be written is silently dropped.
pub struct FileLog {
    dir: PathBuf,
    max_bytes: u64,
    write_lock: Mutex<()>,
}

impl FileLog {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self::with_limit(dir, MAX_BYTES)
    }

    fn with_limit(dir: impl Into<PathBuf>, max_bytes: u64) -> Self {
        Self {
            dir: dir.into(),
            max_bytes,
            write_lock: Mutex::new(()),
        }
    }

    /// Appends `level message` with a UTC timestamp. Newlines in the message
    /// are flattened so one event is one line.
    pub fn write(&self, level: &str, message: &str) {
        let line = format!(
            "{} {level} {}\n",
            utc_stamp(),
            redact_secrets(&message.replace(['\r', '\n'], " "))
        );
        let _guard = self.write_lock.lock();
        if fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let live = self.dir.join(LOG_NAME);
        if fs::metadata(&live).is_ok_and(|meta| meta.len() >= self.max_bytes) {
            let _ = fs::rename(&live, self.dir.join(OLD_LOG_NAME));
        }
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&live) {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

fn read_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

impl LogFiles for FileLog {
    fn dir(&self) -> PathBuf {
        self.dir.clone()
    }

    fn recent_lines(&self, max: usize) -> Result<Vec<String>, AppError> {
        let _guard = self.write_lock.lock();
        let mut lines = read_lines(&self.dir.join(OLD_LOG_NAME));
        lines.extend(read_lines(&self.dir.join(LOG_NAME)));
        let start = lines.len().saturating_sub(max);
        Ok(lines.split_off(start))
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` for now, without a date crate.
fn utc_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);
    format_utc(secs)
}

fn format_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rest = secs.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

/// `SystemInfo` over the real machine.
#[derive(Debug, Clone, Copy, Default)]
pub struct SysinfoOs;

impl SystemInfo for SysinfoOs {
    fn os_version(&self) -> String {
        sysinfo::System::long_os_version()
            .or_else(sysinfo::System::name)
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_timestamped_single_line_events() {
        let dir = tempfile::tempdir().unwrap();
        let log = FileLog::new(dir.path());
        log.write("INFO", "first\nsecond");
        let lines = log.recent_lines(50).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].ends_with("Z INFO first second"), "{}", lines[0]);
    }

    #[test]
    fn never_writes_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let log = FileLog::new(dir.path());
        log.write("WARN", "sign-in failed token=abcdef123456");
        let text = fs::read_to_string(dir.path().join(LOG_NAME)).unwrap();
        assert!(!text.contains("abcdef123456"));
        assert!(text.contains("[redacted]"));
    }

    #[test]
    fn returns_only_the_last_lines_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let log = FileLog::new(dir.path());
        for n in 0..5 {
            log.write("INFO", &format!("event {n}"));
        }
        let lines = log.recent_lines(2).unwrap();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("event 3") && lines[1].ends_with("event 4"));
    }

    #[test]
    fn no_log_yet_is_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        let log = FileLog::new(dir.path().join("missing"));
        assert!(log.recent_lines(50).unwrap().is_empty());
    }

    #[test]
    fn rotates_and_keeps_reading_across_both_files() {
        let dir = tempfile::tempdir().unwrap();
        let log = FileLog::with_limit(dir.path(), 40);
        log.write("INFO", "alpha alpha alpha alpha");
        log.write("INFO", "beta");
        assert!(dir.path().join(OLD_LOG_NAME).exists());
        let lines = log.recent_lines(50).unwrap();
        assert!(lines[0].ends_with("alpha alpha alpha alpha"));
        assert!(lines[1].ends_with("beta"));
    }

    #[test]
    fn formats_the_unix_epoch_and_a_leap_day() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(1_709_210_096), "2024-02-29T12:34:56Z");
    }
}
