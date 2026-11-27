//! Process-wide launcher log (issue #148). The file sink is installed once at
//! startup; before that, and in tests, events only go to stderr.
use motogp_infrastructure::FileLog;
use std::sync::{Arc, OnceLock};

static SINK: OnceLock<Arc<FileLog>> = OnceLock::new();

/// Installs the file sink and a panic hook that records the panic first.
/// A second call keeps the first sink.
pub fn install(log: Arc<FileLog>) {
    if SINK.set(log).is_err() {
        return;
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write("ERROR", &format!("panic: {info}"));
        previous(info);
    }));
}

fn write(level: &str, message: &str) {
    if let Some(log) = SINK.get() {
        log.write(level, message);
    }
}

pub fn info(message: &str) {
    write("INFO", message);
}

/// Logs a problem that did not stop the launcher; also printed to stderr.
pub fn warn(message: &str) {
    eprintln!("{message}");
    write("WARN", message);
}

pub fn error(message: &str) {
    eprintln!("{message}");
    write("ERROR", message);
}
