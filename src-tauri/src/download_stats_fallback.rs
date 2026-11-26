//! Offline stand-in of the download counts (issue #120): used when the HTTP
//! client cannot be built, and as the default of `compose` (tests swap it).
use motogp_application::{DailyDownloads, DownloadStatsApi, DownloadStatsError, ModDownloads};

/// A server that can never be reached.
pub struct UnavailableDownloadStats;

fn offline() -> DownloadStatsError {
    DownloadStatsError("the community server is not available".to_string())
}

impl DownloadStatsApi for UnavailableDownloadStats {
    fn report(&self, _mod_id: &str, _install_id: &str) -> Result<(), DownloadStatsError> {
        Err(offline())
    }

    fn totals(&self) -> Result<Vec<ModDownloads>, DownloadStatsError> {
        Err(offline())
    }

    fn daily(&self, _mod_id: &str) -> Result<Vec<DailyDownloads>, DownloadStatsError> {
        Err(offline())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_call_fails() {
        let api = UnavailableDownloadStats;
        assert!(api.report("a", "i").is_err());
        assert!(api.totals().is_err());
        assert!(api.daily("a").is_err());
    }
}
