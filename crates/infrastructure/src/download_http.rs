//! HTTPS adapter of the mod download (issue #74): streams a catalog file to
//! disk with a hard size cap, progress, cancellation, retries and `Range`
//! resume.
//!
//! This uses the **blocking** reqwest client, so callers must run
//! [`HttpModDownloader::download`] off any async runtime thread (src-tauri
//! uses the blocking pool). The blocking client has no idle timeout of its
//! own, so each attempt reads the body on a short-lived worker thread and the
//! caller side enforces the read timeout and cancellation.
use motogp_application::{
    CancelFlag, DownloadError, DownloadProgress, DownloadRequest, ModDownloader,
};
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE};
use reqwest::redirect::Policy;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

/// Progress is reported at most once per this many received bytes.
const PROGRESS_STEP: u64 = 256 * 1024;
/// Size of one read from the socket.
const CHUNK: usize = 64 * 1024;
/// How often the waiting side checks for cancellation.
const POLL: Duration = Duration::from_millis(100);
/// Safety net so an abandoned worker cannot block forever on a dead socket.
const HARD_TIMEOUT: Duration = Duration::from_secs(6 * 60 * 60);
/// Redirects followed before giving up.
const MAX_REDIRECTS: usize = 5;

/// Settings of [`HttpModDownloader`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpDownloadConfig {
    pub user_agent: String,
    pub connect_timeout: Duration,
    /// Longest silence tolerated while waiting for the server (idle timeout).
    pub read_timeout: Duration,
    /// Retries after the first attempt (so `max_retries + 1` requests at most).
    pub max_retries: u32,
    pub retry_delay: Duration,
    /// Tests only: also accept `http://` URLs that point at the local machine.
    pub allow_http_loopback: bool,
}

impl HttpDownloadConfig {
    /// Production defaults: https only, 10 s to connect, 30 s idle, 3 retries.
    pub fn production(app_version: &str) -> Self {
        Self {
            user_agent: format!(
                "motogp-launcher/{app_version} (+https://github.com/IcaroSilvaFK/motogp-launcher)"
            ),
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_secs(2),
            allow_http_loopback: false,
        }
    }
}

/// [`ModDownloader`] over HTTPS with rustls.
#[derive(Debug)]
pub struct HttpModDownloader {
    client: Client,
    config: HttpDownloadConfig,
}

impl HttpModDownloader {
    /// Builds the HTTP client. Fails only if the TLS backend cannot start.
    pub fn new(config: HttpDownloadConfig) -> Result<Self, DownloadError> {
        // reqwest is built with `rustls-no-provider`: pick ring, as the updater does.
        // An error only means a provider is already installed, which is fine.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let allow_loopback = config.allow_http_loopback;
        let policy = Policy::custom(move |attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                attempt.error("too many redirects")
            } else if is_allowed_url(attempt.url().as_str(), allow_loopback) {
                attempt.follow()
            } else {
                attempt.error("redirect to a non-https URL")
            }
        });
        let client = Client::builder()
            .user_agent(config.user_agent.clone())
            .connect_timeout(config.connect_timeout)
            .timeout(HARD_TIMEOUT)
            .redirect(policy)
            .build()
            .map_err(|e| DownloadError::Network(e.to_string()))?;
        Ok(Self { client, config })
    }
}

/// `https://` always; `http://` only for loopback hosts when allowed.
fn is_allowed_url(url: &str, allow_http_loopback: bool) -> bool {
    if url
        .get(..8)
        .is_some_and(|p| p.eq_ignore_ascii_case("https://"))
    {
        return true;
    }
    let Some(rest) = url
        .get(..7)
        .filter(|p| p.eq_ignore_ascii_case("http://"))
        .map(|_| &url[7..])
    else {
        return false;
    };
    if !allow_http_loopback {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return false;
    }
    let host = match authority.strip_prefix('[') {
        Some(ipv6) => ipv6.split(']').next().unwrap_or(""),
        None => authority.split(':').next().unwrap_or(""),
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

/// What the worker thread reports for one attempt.
enum Msg {
    Head {
        status: u16,
        content_length: Option<u64>,
        range_start: Option<u64>,
    },
    Chunk(Vec<u8>),
    End,
    Failed(String),
}

/// How one attempt ended.
enum Attempt {
    Done,
    /// Worth another try; the text says why.
    Retry(String),
    Fatal(DownloadError),
}

/// Start of a `Content-Range: bytes <start>-<end>/<total>` value.
fn parse_range_start(value: &str) -> Option<u64> {
    value
        .trim()
        .strip_prefix("bytes ")?
        .split('-')
        .next()?
        .trim()
        .parse()
        .ok()
}

fn storage(error: std::io::Error) -> DownloadError {
    DownloadError::Storage(error.to_string())
}

/// Mutable state of one `download` call, shared by its attempts.
struct Transfer<'a> {
    file: File,
    received: u64,
    last_reported: u64,
    max_bytes: u64,
    progress: &'a mut dyn FnMut(DownloadProgress),
    cancel: &'a CancelFlag,
}

impl Transfer<'_> {
    fn restart(&mut self) -> Result<(), DownloadError> {
        self.file.set_len(0).map_err(storage)?;
        self.file.seek(SeekFrom::Start(0)).map_err(storage)?;
        self.received = 0;
        self.last_reported = 0;
        Ok(())
    }

    fn report(&mut self) {
        self.last_reported = self.received;
        (self.progress)(DownloadProgress {
            received: self.received,
            total: self.max_bytes,
        });
    }
}

impl HttpModDownloader {
    /// Starts the request on a worker thread and returns the receiving end.
    fn spawn_request(&self, url: &str, resume_from: u64) -> mpsc::Receiver<Msg> {
        let (tx, rx) = mpsc::sync_channel::<Msg>(4);
        let client = self.client.clone();
        let url = url.to_string();
        thread::spawn(move || {
            let mut request = client.get(&url);
            if resume_from > 0 {
                request = request.header(RANGE, format!("bytes={resume_from}-"));
            }
            let mut response = match request.send() {
                Ok(response) => response,
                Err(error) => {
                    let _ = tx.send(Msg::Failed(error.to_string()));
                    return;
                }
            };
            let header_u64 = |name| {
                response
                    .headers()
                    .get(name)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<u64>().ok())
            };
            let head = Msg::Head {
                status: response.status().as_u16(),
                content_length: header_u64(CONTENT_LENGTH),
                range_start: response
                    .headers()
                    .get(CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok())
                    .and_then(parse_range_start),
            };
            if tx.send(head).is_err() {
                return;
            }
            let mut buf = vec![0u8; CHUNK];
            loop {
                let msg = match response.read(&mut buf) {
                    Ok(0) => Msg::End,
                    Ok(n) => Msg::Chunk(buf[..n].to_vec()),
                    Err(error) => Msg::Failed(error.to_string()),
                };
                let last = !matches!(msg, Msg::Chunk(_));
                if tx.send(msg).is_err() || last {
                    return;
                }
            }
        });
        rx
    }

    fn attempt(&self, url: &str, transfer: &mut Transfer<'_>) -> Attempt {
        let rx = self.spawn_request(url, transfer.received);
        let mut last_activity = Instant::now();
        loop {
            if transfer.cancel.is_cancelled() {
                return Attempt::Fatal(DownloadError::Cancelled);
            }
            let msg = match rx.recv_timeout(POLL) {
                Ok(msg) => msg,
                Err(RecvTimeoutError::Timeout) => {
                    if last_activity.elapsed() >= self.config.read_timeout {
                        return Attempt::Retry("timed out waiting for the server".into());
                    }
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Attempt::Retry("the connection ended unexpectedly".into());
                }
            };
            last_activity = Instant::now();
            match msg {
                Msg::Failed(detail) => return Attempt::Retry(detail),
                Msg::Head {
                    status,
                    content_length,
                    range_start,
                } => {
                    if let Some(outcome) =
                        self.check_head(status, content_length, range_start, transfer)
                    {
                        return outcome;
                    }
                }
                Msg::Chunk(data) => {
                    if let Err(error) = self.write_chunk(&data, transfer) {
                        return Attempt::Fatal(error);
                    }
                }
                Msg::End => {
                    transfer.report();
                    return Attempt::Done;
                }
            }
        }
    }

    /// `Some` when the attempt must stop; `None` to start reading the body.
    fn check_head(
        &self,
        status: u16,
        content_length: Option<u64>,
        range_start: Option<u64>,
        transfer: &mut Transfer<'_>,
    ) -> Option<Attempt> {
        let limit = transfer.max_bytes;
        let too_large = Attempt::Fatal(DownloadError::TooLarge { limit });
        match status {
            200 => {
                if content_length.is_some_and(|len| len > limit) {
                    return Some(too_large);
                }
                // The server ignored the range (or there was none): start over.
                if let Err(error) = transfer.restart() {
                    return Some(Attempt::Fatal(error));
                }
                None
            }
            206 => {
                if transfer.received == 0 || range_start != Some(transfer.received) {
                    return Some(match transfer.restart() {
                        Ok(()) => Attempt::Retry("unexpected partial content".into()),
                        Err(error) => Attempt::Fatal(error),
                    });
                }
                if content_length.is_some_and(|len| transfer.received.saturating_add(len) > limit) {
                    return Some(too_large);
                }
                None
            }
            416 => Some(match transfer.restart() {
                Ok(()) => Attempt::Retry("range not satisfiable".into()),
                Err(error) => Attempt::Fatal(error),
            }),
            408 | 429 | 500..=599 => Some(Attempt::Retry(format!("HTTP status {status}"))),
            _ => Some(Attempt::Fatal(DownloadError::Network(format!(
                "unexpected HTTP status {status}"
            )))),
        }
    }

    fn write_chunk(&self, data: &[u8], transfer: &mut Transfer<'_>) -> Result<(), DownloadError> {
        let next = transfer.received.saturating_add(data.len() as u64);
        if next > transfer.max_bytes {
            return Err(DownloadError::TooLarge {
                limit: transfer.max_bytes,
            });
        }
        transfer.file.write_all(data).map_err(storage)?;
        transfer.received = next;
        if next - transfer.last_reported >= PROGRESS_STEP {
            transfer.report();
        }
        Ok(())
    }

    /// Waits before a retry, staying responsive to cancellation.
    fn pause(&self, cancel: &CancelFlag) -> Result<(), DownloadError> {
        let until = Instant::now() + self.config.retry_delay;
        while Instant::now() < until {
            if cancel.is_cancelled() {
                return Err(DownloadError::Cancelled);
            }
            thread::sleep(POLL.min(until.saturating_duration_since(Instant::now())));
        }
        Ok(())
    }
}

impl ModDownloader for HttpModDownloader {
    fn download(
        &self,
        request: &DownloadRequest,
        dest: &Path,
        progress: &mut dyn FnMut(DownloadProgress),
        cancel: &CancelFlag,
    ) -> Result<(), DownloadError> {
        if !is_allowed_url(&request.url, self.config.allow_http_loopback) {
            return Err(DownloadError::Network(
                "only https downloads are allowed".into(),
            ));
        }
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(dest)
            .map_err(storage)?;
        let mut transfer = Transfer {
            file,
            received: 0,
            last_reported: 0,
            max_bytes: request.max_bytes,
            progress,
            cancel,
        };
        let mut last_error = String::new();
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                self.pause(cancel)?;
            }
            match self.attempt(&request.url, &mut transfer) {
                Attempt::Done => return transfer.file.flush().map_err(storage),
                Attempt::Fatal(error) => return Err(error),
                Attempt::Retry(detail) => last_error = detail,
            }
        }
        Err(DownloadError::Network(format!(
            "{last_error} (gave up after {} attempts)",
            self.config.max_retries + 1
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_is_allowed_in_production() {
        assert!(is_allowed_url("https://example.com/a.zip", false));
        assert!(is_allowed_url("HTTPS://example.com/a.zip", false));
        assert!(!is_allowed_url("http://example.com/a.zip", false));
        assert!(!is_allowed_url("http://127.0.0.1:80/a.zip", false));
        assert!(!is_allowed_url("ftp://example.com/a.zip", false));
        assert!(!is_allowed_url("", false));
    }

    #[test]
    fn http_is_allowed_only_for_loopback_when_enabled() {
        assert!(is_allowed_url("http://127.0.0.1:8080/a.zip", true));
        assert!(is_allowed_url("http://localhost/a.zip", true));
        assert!(is_allowed_url("http://[::1]:9/a.zip", true));
        assert!(!is_allowed_url("http://example.com/a.zip", true));
        assert!(!is_allowed_url("http://127.0.0.1@example.com/a.zip", true));
        assert!(!is_allowed_url("http://127.0.0.1.example.com/a.zip", true));
    }

    #[test]
    fn content_range_start_is_parsed() {
        assert_eq!(parse_range_start("bytes 100-199/200"), Some(100));
        assert_eq!(parse_range_start("bytes */200"), None);
        assert_eq!(parse_range_start("items 1-2/3"), None);
    }

    #[test]
    fn production_defaults_match_the_spec() {
        let config = HttpDownloadConfig::production("1.2.3");
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.read_timeout, Duration::from_secs(30));
        assert_eq!(config.max_retries, 3);
        assert!(!config.allow_http_loopback);
        assert!(config.user_agent.starts_with("motogp-launcher/1.2.3 "));
    }
}
