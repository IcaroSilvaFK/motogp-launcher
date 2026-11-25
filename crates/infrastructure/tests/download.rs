//! Edge cases of the HTTP mod downloader (issue #74). Local 127.0.0.1 server only.
#[path = "support/http_server.rs"]
mod http_server;

use http_server::{Reply, TestServer};
use motogp_application::{
    CancelFlag, DownloadError, DownloadProgress, DownloadRequest, ModDownloader,
};
use motogp_infrastructure::{HttpDownloadConfig, HttpModDownloader};
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

const SIZE: usize = 1000;

fn data() -> Vec<u8> {
    (0..SIZE).map(|i| (i % 251) as u8).collect()
}

fn config(max_retries: u32) -> HttpDownloadConfig {
    HttpDownloadConfig {
        user_agent: "motogp-launcher-tests".into(),
        connect_timeout: Duration::from_secs(5),
        read_timeout: Duration::from_secs(5),
        max_retries,
        retry_delay: Duration::from_millis(1),
        allow_http_loopback: true,
    }
}

struct Run {
    outcome: Result<(), DownloadError>,
    file: Vec<u8>,
    progress: Vec<DownloadProgress>,
    server: TestServer,
}

fn run_with(replies: Vec<Reply>, max_retries: u32, cancel: &CancelFlag) -> Run {
    let server = TestServer::start(replies);
    let tmp = TempDir::new().unwrap();
    let dest = tmp.path().join("mod.part");
    let downloader = HttpModDownloader::new(config(max_retries)).unwrap();
    let mut progress = Vec::new();
    let outcome = downloader.download(
        &DownloadRequest {
            url: server.url.clone(),
            max_bytes: SIZE as u64,
        },
        &dest,
        &mut |p| progress.push(p),
        cancel,
    );
    let file = std::fs::read(&dest).unwrap_or_default();
    Run {
        outcome,
        file,
        progress,
        server,
    }
}

fn run(replies: Vec<Reply>, max_retries: u32) -> Run {
    run_with(replies, max_retries, &CancelFlag::new())
}

#[test]
fn a_plain_download_reports_final_progress() {
    let r = run(vec![Reply::new(200).body(data())], 0);
    assert_eq!(r.outcome, Ok(()));
    assert_eq!(r.file, data());
    assert_eq!(
        r.progress.last(),
        Some(&DownloadProgress {
            received: SIZE as u64,
            total: SIZE as u64
        })
    );
    let request = &r.server.requests()[0];
    assert_eq!(request.header("user-agent"), Some("motogp-launcher-tests"));
}

#[test]
fn a_cancelled_flag_stops_before_downloading() {
    let cancel = CancelFlag::new();
    cancel.cancel();
    let r = run_with(vec![Reply::new(200).body(data())], 3, &cancel);
    assert_eq!(r.outcome, Err(DownloadError::Cancelled));
}

#[test]
fn a_client_error_fails_at_once_without_retrying() {
    let r = run(vec![Reply::new(404)], 3);
    assert!(matches!(r.outcome, Err(DownloadError::Network(_))));
    assert_eq!(r.server.requests().len(), 1);
}

#[test]
fn a_server_error_is_retried() {
    let r = run(vec![Reply::new(503), Reply::new(200).body(data())], 3);
    assert_eq!(r.outcome, Ok(()));
    assert_eq!(r.file, data());
    assert_eq!(r.server.requests().len(), 2);
}

#[test]
fn too_many_requests_is_retried() {
    let r = run(vec![Reply::new(429), Reply::new(200).body(data())], 1);
    assert_eq!(r.outcome, Ok(()));
}

#[test]
fn a_server_that_ignores_range_restarts_from_scratch() {
    let half = SIZE / 2;
    let dropped = Reply::new(200)
        .declared_len(SIZE)
        .body(data()[..half].to_vec());
    let full = Reply::new(200).body(data());
    let r = run(vec![dropped, full], 3);
    assert_eq!(r.outcome, Ok(()));
    assert_eq!(r.file, data());
    assert_eq!(r.server.requests()[1].header("range"), Some("bytes=500-"));
}

#[test]
fn range_not_satisfiable_restarts_from_scratch() {
    let half = SIZE / 2;
    let dropped = Reply::new(200)
        .declared_len(SIZE)
        .body(data()[..half].to_vec());
    let r = run(
        vec![dropped, Reply::new(416), Reply::new(200).body(data())],
        3,
    );
    assert_eq!(r.outcome, Ok(()));
    assert_eq!(r.file, data());
    assert_eq!(r.server.requests()[2].header("range"), None);
}

#[test]
fn a_partial_reply_for_the_wrong_offset_restarts_from_scratch() {
    let half = SIZE / 2;
    let dropped = Reply::new(200)
        .declared_len(SIZE)
        .body(data()[..half].to_vec());
    let wrong = Reply::new(206)
        .header("Content-Range", "bytes 0-999/1000")
        .body(data());
    let r = run(vec![dropped, wrong, Reply::new(200).body(data())], 3);
    assert_eq!(r.outcome, Ok(()));
    assert_eq!(r.file, data());
}

#[test]
fn a_partial_reply_that_overshoots_the_cap_is_too_large() {
    let half = SIZE / 2;
    let dropped = Reply::new(200)
        .declared_len(SIZE)
        .body(data()[..half].to_vec());
    let too_big = Reply::new(206)
        .header("Content-Range", "bytes 500-1099/1100")
        .body(vec![0u8; 600]);
    let r = run(vec![dropped, too_big], 3);
    assert_eq!(
        r.outcome,
        Err(DownloadError::TooLarge { limit: SIZE as u64 })
    );
}

#[test]
fn an_announced_length_over_the_cap_fails_before_reading() {
    let r = run(vec![Reply::new(200).declared_len(SIZE + 1)], 3);
    assert_eq!(
        r.outcome,
        Err(DownloadError::TooLarge { limit: SIZE as u64 })
    );
    assert_eq!(r.server.requests().len(), 1);
}

#[test]
fn an_http_url_is_refused_without_loopback_permission() {
    let server = TestServer::start(vec![Reply::new(200).body(data())]);
    let mut strict = config(3);
    strict.allow_http_loopback = false;
    let downloader = HttpModDownloader::new(strict).unwrap();
    let tmp = TempDir::new().unwrap();
    let dest = tmp.path().join("mod.part");
    let outcome = downloader.download(
        &DownloadRequest {
            url: server.url.clone(),
            max_bytes: SIZE as u64,
        },
        &dest,
        &mut |_| {},
        &CancelFlag::new(),
    );
    assert!(matches!(outcome, Err(DownloadError::Network(_))));
    assert!(server.requests().is_empty());
    assert!(!dest.exists());
}

#[test]
fn a_redirect_to_plain_http_elsewhere_is_refused() {
    let redirect = Reply::new(302).header("Location", "http://example.com/mod.zip");
    let r = run(vec![redirect], 0);
    assert!(matches!(r.outcome, Err(DownloadError::Network(_))));
    assert_eq!(r.server.requests().len(), 1);
}

#[test]
fn an_unwritable_destination_is_a_storage_error() {
    let server = TestServer::start(vec![Reply::new(200).body(data())]);
    let downloader = HttpModDownloader::new(config(0)).unwrap();
    let tmp = TempDir::new().unwrap();
    let dest: PathBuf = tmp.path().join("missing-dir").join("mod.part");
    let outcome = downloader.download(
        &DownloadRequest {
            url: server.url.clone(),
            max_bytes: SIZE as u64,
        },
        &dest,
        &mut |_| {},
        &CancelFlag::new(),
    );
    assert!(matches!(outcome, Err(DownloadError::Storage(_))));
    assert!(server.requests().is_empty());
}

#[test]
fn a_silent_server_hits_the_idle_timeout_and_gives_up() {
    // Accepts the connection, reads the request and never answers.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/mod.zip", listener.local_addr().unwrap());
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let handle = std::thread::spawn(move || {
        let mut held = Vec::new();
        listener.set_nonblocking(true).unwrap();
        while stop_rx.try_recv().is_err() {
            if let Ok((stream, _)) = listener.accept() {
                held.push(stream);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    let mut quick = config(1);
    quick.read_timeout = Duration::from_millis(300);
    let downloader = HttpModDownloader::new(quick).unwrap();
    let tmp = TempDir::new().unwrap();
    let outcome = downloader.download(
        &DownloadRequest {
            url,
            max_bytes: SIZE as u64,
        },
        &tmp.path().join("mod.part"),
        &mut |_| {},
        &CancelFlag::new(),
    );
    let _ = stop_tx.send(());
    handle.join().unwrap();
    match outcome {
        Err(DownloadError::Network(detail)) => assert!(detail.contains("timed out"), "{detail}"),
        other => panic!("expected a timeout, got {other:?}"),
    }
}

#[test]
fn cancelling_while_the_server_is_silent_is_prompt() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/mod.zip", listener.local_addr().unwrap());
    let cancel = CancelFlag::new();
    let trigger = cancel.clone();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        trigger.cancel();
    });
    let downloader = HttpModDownloader::new(config(3)).unwrap();
    let tmp = TempDir::new().unwrap();
    let started = std::time::Instant::now();
    let outcome = downloader.download(
        &DownloadRequest {
            url,
            max_bytes: SIZE as u64,
        },
        &tmp.path().join("mod.part"),
        &mut |_| {},
        &cancel,
    );
    canceller.join().unwrap();
    drop(listener);
    assert_eq!(outcome, Err(DownloadError::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(3));
}
