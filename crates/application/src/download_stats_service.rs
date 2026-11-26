//! Anonymous download counts (issue #120). Every failure becomes "nothing":
//! the launcher shows no count and no error when the server is out of reach.
use crate::{DailyDownloads, DownloadStatsApi, InstallIdStore, ModDownloads};
use std::sync::Arc;

/// Buckets of the daily answer: today and the 29 days before.
const DAILY_BUCKETS: usize = 30;
/// Longest catalog id the server accepts.
const MAX_ID_LEN: usize = 64;

/// A catalog slug: 1 to 64 characters of `a-z 0-9 -` (what the server accepts).
fn is_catalog_id(id: &str) -> bool {
    (1..=MAX_ID_LEN).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub struct DownloadStatsService {
    api: Arc<dyn DownloadStatsApi>,
    ids: Arc<dyn InstallIdStore>,
}

impl DownloadStatsService {
    pub fn new(api: Arc<dyn DownloadStatsApi>, ids: Arc<dyn InstallIdStore>) -> Self {
        Self { api, ids }
    }

    /// Reports one install. Skipped when `mod_id` is not a catalog id or the
    /// install id cannot be obtained; a server failure is ignored.
    pub fn report(&self, mod_id: &str) {
        if !is_catalog_id(mod_id) {
            return;
        }
        let Ok(install_id) = self.ids.load_or_create() else {
            return;
        };
        let _ = self.api.report(mod_id, &install_id);
    }

    /// Totals per mod, or `None` when the server cannot be reached.
    pub fn totals(&self) -> Option<Vec<ModDownloads>> {
        self.api.totals().ok()
    }

    /// The 30 daily buckets of a mod, or `None` for anything else.
    pub fn daily(&self, mod_id: &str) -> Option<Vec<DailyDownloads>> {
        if !is_catalog_id(mod_id) {
            return None;
        }
        self.api
            .daily(mod_id)
            .ok()
            .filter(|days| days.len() == DAILY_BUCKETS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DownloadStatsError;
    use std::sync::Mutex;

    fn failure() -> DownloadStatsError {
        DownloadStatsError("down".to_string())
    }

    #[derive(Default)]
    struct FakeApi {
        reports: Mutex<Vec<(String, String)>>,
        totals: Mutex<Option<Vec<ModDownloads>>>,
        daily: Mutex<Option<Vec<DailyDownloads>>>,
    }

    impl DownloadStatsApi for FakeApi {
        fn report(&self, mod_id: &str, install_id: &str) -> Result<(), DownloadStatsError> {
            self.reports
                .lock()
                .unwrap()
                .push((mod_id.to_string(), install_id.to_string()));
            Ok(())
        }
        fn totals(&self) -> Result<Vec<ModDownloads>, DownloadStatsError> {
            self.totals.lock().unwrap().clone().ok_or_else(failure)
        }
        fn daily(&self, _mod_id: &str) -> Result<Vec<DailyDownloads>, DownloadStatsError> {
            self.daily.lock().unwrap().clone().ok_or_else(failure)
        }
    }

    struct FailingReport;

    impl DownloadStatsApi for FailingReport {
        fn report(&self, _: &str, _: &str) -> Result<(), DownloadStatsError> {
            Err(failure())
        }
        fn totals(&self) -> Result<Vec<ModDownloads>, DownloadStatsError> {
            Err(failure())
        }
        fn daily(&self, _: &str) -> Result<Vec<DailyDownloads>, DownloadStatsError> {
            Err(failure())
        }
    }

    struct FixedId(Option<&'static str>);

    impl InstallIdStore for FixedId {
        fn load_or_create(&self) -> Result<String, DownloadStatsError> {
            self.0.map(str::to_string).ok_or_else(failure)
        }
    }

    fn service(api: Arc<FakeApi>) -> DownloadStatsService {
        DownloadStatsService::new(api, Arc::new(FixedId(Some("install-1"))))
    }

    fn days(count: usize) -> Vec<DailyDownloads> {
        (0..count)
            .map(|i| DailyDownloads {
                date: format!("2026-10-{:02}", i + 1),
                count: i as u64,
            })
            .collect()
    }

    #[test]
    fn report_sends_the_mod_id_with_the_install_id() {
        let api = Arc::new(FakeApi::default());
        service(api.clone()).report("livery-pack");
        assert_eq!(
            *api.reports.lock().unwrap(),
            vec![("livery-pack".to_string(), "install-1".to_string())]
        );
    }

    #[test]
    fn report_skips_ids_that_are_not_catalog_slugs() {
        let api = Arc::new(FakeApi::default());
        let service = service(api.clone());
        for bad in ["", "Upper", "a b", "a/b", "ação", &"a".repeat(65)] {
            service.report(bad);
        }
        assert!(api.reports.lock().unwrap().is_empty());
    }

    #[test]
    fn report_accepts_a_64_character_id() {
        let api = Arc::new(FakeApi::default());
        service(api.clone()).report(&"a".repeat(64));
        assert_eq!(api.reports.lock().unwrap().len(), 1);
    }

    #[test]
    fn report_skips_when_the_install_id_is_unavailable() {
        let api = Arc::new(FakeApi::default());
        DownloadStatsService::new(api.clone(), Arc::new(FixedId(None))).report("a");
        assert!(api.reports.lock().unwrap().is_empty());
    }

    #[test]
    fn report_swallows_a_server_failure() {
        DownloadStatsService::new(Arc::new(FailingReport), Arc::new(FixedId(Some("i"))))
            .report("a");
    }

    #[test]
    fn totals_pass_through_and_fail_to_none() {
        let api = Arc::new(FakeApi::default());
        let service = service(api.clone());
        assert_eq!(service.totals(), None);
        let totals = vec![ModDownloads {
            id: "a".to_string(),
            total: 5,
            last_30_days: 2,
        }];
        *api.totals.lock().unwrap() = Some(totals.clone());
        assert_eq!(service.totals(), Some(totals));
    }

    #[test]
    fn daily_needs_exactly_thirty_buckets() {
        let api = Arc::new(FakeApi::default());
        let service = service(api.clone());
        assert_eq!(service.daily("a"), None);
        for wrong in [0, 29, 31] {
            *api.daily.lock().unwrap() = Some(days(wrong));
            assert_eq!(service.daily("a"), None, "{wrong} buckets");
        }
        *api.daily.lock().unwrap() = Some(days(30));
        assert_eq!(service.daily("a"), Some(days(30)));
    }

    #[test]
    fn daily_rejects_a_bad_id_without_calling_the_server() {
        let api = Arc::new(FakeApi::default());
        *api.daily.lock().unwrap() = Some(days(30));
        assert_eq!(service(api).daily("Bad Id"), None);
    }
}
