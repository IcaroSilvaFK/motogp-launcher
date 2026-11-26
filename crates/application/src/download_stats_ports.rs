//! Ports of the anonymous download counts (issue #120).
use std::fmt;

/// Download totals of one mod, as `GET /mods/downloads` returns them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModDownloads {
    pub id: String,
    pub total: u64,
    pub last_30_days: u64,
}

/// One UTC day of downloads, `date` as `YYYY-MM-DD`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyDownloads {
    pub date: String,
    pub count: u64,
}

/// Why a download-stats call failed. Callers never show it: the feature is
/// silent by design. The text never carries the install id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadStatsError(pub String);

impl fmt::Display for DownloadStatsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DownloadStatsError {}

/// Port: the community server's anonymous download endpoints.
pub trait DownloadStatsApi: Send + Sync {
    /// Counts one install of `mod_id` for this install id (once per UTC day).
    fn report(&self, mod_id: &str, install_id: &str) -> Result<(), DownloadStatsError>;
    /// Totals of every mod with at least one download.
    fn totals(&self) -> Result<Vec<ModDownloads>, DownloadStatsError>;
    /// The last 30 UTC days of one mod, oldest first.
    fn daily(&self, mod_id: &str) -> Result<Vec<DailyDownloads>, DownloadStatsError>;
}

/// Port: the random id of this install, created once and kept between launches.
pub trait InstallIdStore: Send + Sync {
    /// Returns the stored id, creating and saving a new one when there is none.
    fn load_or_create(&self) -> Result<String, DownloadStatsError>;
}
