//! Steps for `tests/features/mod-download.feature` (issue #74).
//! A local 127.0.0.1 server and temporary directories only: never the internet.
use crate::feature;
use cucumber::writer::Stats as _;
use cucumber::{given, then, when, World};
use motogp_application::{
    CancelFlag, DownloadError, DownloadProgress, DownloadRequest, ModDownloader,
};
use motogp_infrastructure::{HttpDownloadConfig, HttpModDownloader};
use std::time::Duration;
use tempfile::TempDir;

use crate::catalog_source::http_server::{Reply, TestServer};

const MIB: usize = 1024 * 1024;

#[derive(Debug, Default, World)]
struct DownloadWorld {
    server: Option<TestServer>,
    served: Vec<u8>,
    tmp: Option<TempDir>,
    outcome: Option<Result<(), DownloadError>>,
    progress: Vec<DownloadProgress>,
}

/// Deterministic, non-repeating-looking bytes.
fn payload(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i.wrapping_mul(31) % 251) as u8).collect()
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

impl DownloadWorld {
    fn dest(&mut self) -> std::path::PathBuf {
        self.tmp
            .get_or_insert_with(|| TempDir::new().unwrap())
            .path()
            .join("mod.zip.part")
    }

    fn run(&mut self, url: String, max_bytes: u64, config: HttpDownloadConfig) {
        let downloader = HttpModDownloader::new(config).expect("client");
        let dest = self.dest();
        let mut progress = Vec::new();
        let outcome = downloader.download(
            &DownloadRequest { url, max_bytes },
            &dest,
            &mut |p| progress.push(p),
            &CancelFlag::new(),
        );
        self.progress = progress;
        self.outcome = Some(outcome);
    }

    fn run_against_server(&mut self, max_bytes: u64, max_retries: u32) {
        let url = self.server.as_ref().expect("no server").url.clone();
        self.run(url, max_bytes, config(max_retries));
    }

    fn server(&self) -> &TestServer {
        self.server.as_ref().expect("no server")
    }

    fn error(&self) -> &DownloadError {
        self.outcome
            .as_ref()
            .expect("no download")
            .as_ref()
            .expect_err("expected a failure")
    }
}

#[given("a local server serves a 1 MiB file")]
fn serves_one_mib(w: &mut DownloadWorld) {
    w.served = payload(MIB);
    w.server = Some(TestServer::start(vec![
        Reply::new(200).body(w.served.clone())
    ]));
}

#[given("a local server drops the connection after half of the file")]
fn drops_halfway(w: &mut DownloadWorld) {
    w.served = payload(MIB);
    let half = MIB / 2;
    let dropped = Reply::new(200)
        .declared_len(MIB)
        .body(w.served[..half].to_vec());
    let rest = Reply::new(206)
        .header("Content-Range", &format!("bytes {half}-{}/{MIB}", MIB - 1))
        .body(w.served[half..].to_vec());
    w.server = Some(TestServer::start(vec![dropped, rest]));
}

#[given("a local server serves more bytes than the cap")]
fn serves_too_much(w: &mut DownloadWorld) {
    w.served = payload(MIB + 100);
    w.server = Some(TestServer::start(vec![
        Reply::new(200).body(w.served.clone())
    ]));
}

#[given("a local server that always drops the connection")]
fn always_drops(w: &mut DownloadWorld) {
    w.server = Some(TestServer::start(vec![Reply::new(200).declared_len(MIB)]));
}

#[when("it is downloaded with a 1 MiB cap")]
fn with_cap(w: &mut DownloadWorld) {
    w.run_against_server(MIB as u64, 0);
}

#[when("it is downloaded")]
fn downloaded(w: &mut DownloadWorld) {
    w.run_against_server(MIB as u64, 3);
}

#[when(expr = "it is downloaded with {int} retries")]
fn with_retries(w: &mut DownloadWorld, retries: u32) {
    w.run_against_server(MIB as u64, retries);
}

#[when(expr = "a download from {string} is requested")]
fn requested(w: &mut DownloadWorld, url: String) {
    // A server is running, but production rules must not even try the URL.
    w.server = Some(TestServer::start(vec![Reply::new(200).body(b"x".to_vec())]));
    let mut production = config(3);
    production.allow_http_loopback = false;
    w.run(url, MIB as u64, production);
}

#[then("the file on disk matches the served bytes")]
fn file_matches(w: &mut DownloadWorld) {
    assert_eq!(w.outcome, Some(Ok(())));
    let on_disk = std::fs::read(w.dest()).expect("file written");
    assert!(
        on_disk == w.served,
        "downloaded bytes differ from the served bytes"
    );
}

#[then("progress was reported up to 1 MiB")]
fn progress_reported(w: &mut DownloadWorld) {
    let last = w.progress.last().expect("no progress reported");
    assert_eq!(last.received, MIB as u64);
    assert_eq!(last.total, MIB as u64);
    assert!(w.progress.len() > 1, "expected intermediate progress too");
}

#[then("the second request asks for the remaining bytes with a Range header")]
fn range_header(w: &mut DownloadWorld) {
    let requests = w.server().requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].header("range"), None);
    assert_eq!(
        requests[1].header("range"),
        Some(format!("bytes={}-", MIB / 2).as_str())
    );
}

#[then("the download fails as too large")]
fn too_large(w: &mut DownloadWorld) {
    assert_eq!(w.error(), &DownloadError::TooLarge { limit: MIB as u64 });
}

#[then("the download fails with a network error after 4 attempts")]
fn gave_up(w: &mut DownloadWorld) {
    assert!(matches!(w.error(), DownloadError::Network(_)));
    assert_eq!(w.server().requests().len(), 4);
}

#[then("it fails without any request")]
fn refused(w: &mut DownloadWorld) {
    assert!(matches!(w.error(), DownloadError::Network(_)));
    assert!(w.server().requests().is_empty());
}

/// Runs only `mod-download.feature`; returns `true` when every step passed.
pub async fn run() -> bool {
    let writer = DownloadWorld::cucumber()
        .fail_on_skipped()
        .run(feature("mod-download.feature"))
        .await;
    !writer.execution_has_failed()
}
