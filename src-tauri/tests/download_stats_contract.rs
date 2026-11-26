//! IPC contract of the anonymous download counts (#120) through the Tauri mock
//! runtime with a fake server port: never the network.
mod ipc_support;

use ipc_support::Harness;
use motogp_application::{
    DailyDownloads, DownloadStatsApi, DownloadStatsError, DownloadStatsService, InstallIdStore,
    ModDownloads,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeServer {
    online: bool,
    reports: Mutex<Vec<(String, String)>>,
}

fn down() -> DownloadStatsError {
    DownloadStatsError("down".to_string())
}

impl DownloadStatsApi for FakeServer {
    fn report(&self, mod_id: &str, install_id: &str) -> Result<(), DownloadStatsError> {
        self.reports
            .lock()
            .unwrap()
            .push((mod_id.to_string(), install_id.to_string()));
        if self.online {
            Ok(())
        } else {
            Err(down())
        }
    }
    fn totals(&self) -> Result<Vec<ModDownloads>, DownloadStatsError> {
        if !self.online {
            return Err(down());
        }
        Ok(vec![ModDownloads {
            id: "livery-pack".to_string(),
            total: 101,
            last_30_days: 7,
        }])
    }
    fn daily(&self, _mod_id: &str) -> Result<Vec<DailyDownloads>, DownloadStatsError> {
        if !self.online {
            return Err(down());
        }
        Ok((1..=30)
            .map(|day| DailyDownloads {
                date: format!("2026-11-{day:02}"),
                count: day as u64 % 3,
            })
            .collect())
    }
}

struct FixedInstallId;

impl InstallIdStore for FixedInstallId {
    fn load_or_create(&self) -> Result<String, DownloadStatsError> {
        Ok("install-id-for-tests".to_string())
    }
}

fn harness(online: bool) -> (Harness, Arc<FakeServer>) {
    let server = Arc::new(FakeServer {
        online,
        ..Default::default()
    });
    let service = DownloadStatsService::new(server.clone(), Arc::new(FixedInstallId));
    (Harness::with_downloads(Arc::new(service)), server)
}

#[test]
fn get_mod_downloads_answers_camel_case_totals() {
    let (h, _) = harness(true);
    assert_eq!(
        h.invoke("get_mod_downloads", json!({})).unwrap(),
        json!([{ "id": "livery-pack", "total": 101, "last30Days": 7 }])
    );
}

#[test]
fn get_mod_download_daily_answers_30_dated_buckets() {
    let (h, _) = harness(true);
    let days = h
        .invoke("get_mod_download_daily", json!({ "id": "livery-pack" }))
        .unwrap();
    let days = days.as_array().unwrap();
    assert_eq!(days.len(), 30);
    assert_eq!(days[0], json!({ "date": "2026-11-01", "count": 1 }));
    assert_eq!(days[29], json!({ "date": "2026-11-30", "count": 0 }));
}

#[test]
fn an_unreachable_server_answers_null_never_an_error() {
    let (h, _) = harness(false);
    assert_eq!(
        h.invoke("get_mod_downloads", json!({})).unwrap(),
        Value::Null
    );
    assert_eq!(
        h.invoke("get_mod_download_daily", json!({ "id": "livery-pack" }))
            .unwrap(),
        Value::Null
    );
}

#[test]
fn a_bad_mod_id_answers_null() {
    let (h, _) = harness(true);
    assert_eq!(
        h.invoke("get_mod_download_daily", json!({ "id": "Not A Slug" }))
            .unwrap(),
        Value::Null
    );
}

#[test]
fn report_mod_download_sends_the_mod_and_install_id_and_answers_nothing() {
    let (h, server) = harness(true);
    assert_eq!(
        h.invoke("report_mod_download", json!({ "id": "livery-pack" }))
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        *server.reports.lock().unwrap(),
        vec![(
            "livery-pack".to_string(),
            "install-id-for-tests".to_string()
        )]
    );
}

#[test]
fn report_mod_download_never_errors_when_the_server_is_down() {
    let (h, _) = harness(false);
    assert_eq!(
        h.invoke("report_mod_download", json!({ "id": "livery-pack" }))
            .unwrap(),
        Value::Null
    );
}

#[test]
fn report_mod_download_skips_a_bad_id() {
    let (h, server) = harness(true);
    h.invoke("report_mod_download", json!({ "id": "../etc" }))
        .unwrap();
    assert!(server.reports.lock().unwrap().is_empty());
}

#[test]
fn the_default_composition_is_offline_and_silent() {
    let h = Harness::new();
    assert_eq!(
        h.invoke("get_mod_downloads", json!({})).unwrap(),
        Value::Null
    );
    h.invoke("report_mod_download", json!({ "id": "a" }))
        .unwrap();
}
