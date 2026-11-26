//! Download counts: one count per (mod, install, UTC day).
//!
//! Only the SHA-256 of the install id is stored, never the raw id.

use std::sync::Arc;

use community_domain::downloads::{day_of, fill_window, window_start, DayBucket};
use community_domain::{InstallId, ModSlug};

use crate::ports::{Clock, Hasher, PortResult};
use crate::AppError;

/// Download totals of one mod.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModDownloads {
    pub id: String,
    pub total: u64,
    /// Counts in the 30-day window (today and the 29 days before).
    pub last_30_days: u64,
}

#[async_trait::async_trait]
pub trait DownloadRepository: Send + Sync {
    /// Records one download; a repeat of the same triple is a no-op.
    async fn record(&self, mod_id: &str, install_hash: &str, day: i64) -> PortResult<()>;
    /// Every mod with at least one download, sorted by id. `last_30_days`
    /// counts the days `since_day` and later.
    async fn totals(&self, since_day: i64) -> PortResult<Vec<ModDownloads>>;
    /// Non-empty days of a mod within `from_day..=to_day`, as `(day, count)`.
    async fn daily(&self, mod_id: &str, from_day: i64, to_day: i64) -> PortResult<Vec<(i64, u64)>>;
}

pub struct DownloadService {
    pub clock: Arc<dyn Clock>,
    pub hasher: Arc<dyn Hasher>,
    pub downloads: Arc<dyn DownloadRepository>,
}

impl DownloadService {
    /// Counts a download. `BadRequest` for a malformed mod id or install id.
    pub async fn report(&self, mod_id: &str, install_id: &str) -> Result<(), AppError> {
        let slug = parse_slug(mod_id)?;
        let install = InstallId::parse(install_id)
            .ok_or_else(|| AppError::BadRequest("The install id is invalid.".into()))?;
        let hash = self.hasher.hash(install.expose());
        let today = day_of(self.clock.now());
        self.downloads.record(slug.as_str(), &hash, today).await
    }

    /// Totals of every mod that has downloads, sorted by id.
    pub async fn totals(&self) -> Result<Vec<ModDownloads>, AppError> {
        let today = day_of(self.clock.now());
        self.downloads.totals(window_start(today)).await
    }

    /// The 30 daily buckets of a mod (zeros for an unknown mod).
    pub async fn daily(&self, mod_id: &str) -> Result<Vec<DayBucket>, AppError> {
        let slug = parse_slug(mod_id)?;
        let today = day_of(self.clock.now());
        let counts = self
            .downloads
            .daily(slug.as_str(), window_start(today), today)
            .await?;
        Ok(fill_window(today, &counts))
    }
}

fn parse_slug(mod_id: &str) -> Result<ModSlug, AppError> {
    ModSlug::parse(mod_id).ok_or_else(|| AppError::BadRequest("The mod id is invalid.".into()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Mutex;

    use chrono::{DateTime, Utc};

    use super::*;

    struct Fixed(Mutex<i64>);
    impl Clock for Fixed {
        fn now(&self) -> DateTime<Utc> {
            DateTime::from_timestamp(*self.0.lock().unwrap(), 0).unwrap()
        }
    }

    struct FakeHasher;
    impl Hasher for FakeHasher {
        fn hash(&self, input: &str) -> String {
            format!("h:{input}")
        }
    }

    #[derive(Default)]
    struct FakeRepo(Mutex<BTreeSet<(String, String, i64)>>);

    #[async_trait::async_trait]
    impl DownloadRepository for FakeRepo {
        async fn record(&self, mod_id: &str, install_hash: &str, day: i64) -> PortResult<()> {
            self.0
                .lock()
                .unwrap()
                .insert((mod_id.into(), install_hash.into(), day));
            Ok(())
        }
        async fn totals(&self, since_day: i64) -> PortResult<Vec<ModDownloads>> {
            let rows = self.0.lock().unwrap();
            let mut ids: Vec<&String> = rows.iter().map(|r| &r.0).collect();
            ids.dedup();
            Ok(ids
                .into_iter()
                .map(|id| ModDownloads {
                    id: id.clone(),
                    total: rows.iter().filter(|r| &r.0 == id).count() as u64,
                    last_30_days: rows
                        .iter()
                        .filter(|r| &r.0 == id && r.2 >= since_day)
                        .count() as u64,
                })
                .collect())
        }
        async fn daily(&self, mod_id: &str, from: i64, to: i64) -> PortResult<Vec<(i64, u64)>> {
            let rows = self.0.lock().unwrap();
            let mut out: Vec<(i64, u64)> = Vec::new();
            for r in rows
                .iter()
                .filter(|r| r.0 == mod_id && (from..=to).contains(&r.2))
            {
                match out.iter_mut().find(|(d, _)| *d == r.2) {
                    Some(entry) => entry.1 += 1,
                    None => out.push((r.2, 1)),
                }
            }
            Ok(out)
        }
    }

    const DAY: i64 = 86_400;

    fn service() -> (Arc<Fixed>, Arc<FakeRepo>, DownloadService) {
        let clock = Arc::new(Fixed(Mutex::new(20_000 * DAY + 100)));
        let repo = Arc::new(FakeRepo::default());
        let service = DownloadService {
            clock: clock.clone(),
            hasher: Arc::new(FakeHasher),
            downloads: repo.clone(),
        };
        (clock, repo, service)
    }

    fn install(n: char) -> String {
        n.to_string().repeat(22)
    }

    #[tokio::test]
    async fn the_same_install_counts_once_per_day() {
        let (clock, _, service) = service();
        service.report("mod-a", &install('a')).await.unwrap();
        service.report("mod-a", &install('a')).await.unwrap();
        assert_eq!(service.totals().await.unwrap()[0].total, 1);
        *clock.0.lock().unwrap() += DAY;
        service.report("mod-a", &install('a')).await.unwrap();
        let totals = service.totals().await.unwrap();
        assert_eq!((totals[0].total, totals[0].last_30_days), (2, 2));
    }

    #[tokio::test]
    async fn only_the_hash_reaches_the_repository() {
        let (_, repo, service) = service();
        service.report("mod-a", &install('a')).await.unwrap();
        let rows = repo.0.lock().unwrap();
        let (_, hash, _) = rows.iter().next().unwrap();
        assert_eq!(hash, &format!("h:{}", install('a')));
    }

    #[tokio::test]
    async fn invalid_ids_are_bad_requests_and_nothing_is_stored() {
        let (_, repo, service) = service();
        for (m, i) in [("Bad Mod", install('a')), ("mod-a", "short".to_owned())] {
            assert!(matches!(
                service.report(m, &i).await,
                Err(AppError::BadRequest(_))
            ));
        }
        assert!(repo.0.lock().unwrap().is_empty());
        assert!(matches!(
            service.daily("../x").await,
            Err(AppError::BadRequest(_))
        ));
    }

    #[tokio::test]
    async fn daily_is_zero_filled_and_ends_today() {
        let (_, _, service) = service();
        service.report("mod-a", &install('a')).await.unwrap();
        service.report("mod-a", &install('b')).await.unwrap();
        let days = service.daily("mod-a").await.unwrap();
        assert_eq!(days.len(), 30);
        assert_eq!(days[29].day, 20_000);
        assert_eq!(days[29].count, 2);
        assert_eq!(days.iter().map(|d| d.count).sum::<u64>(), 2);
        let unknown = service.daily("nope").await.unwrap();
        assert_eq!(unknown.len(), 30);
        assert!(unknown.iter().all(|d| d.count == 0));
    }
}
