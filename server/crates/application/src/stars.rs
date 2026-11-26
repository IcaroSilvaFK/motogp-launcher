//! Stars use cases: one star per (user, mod), idempotent star/unstar.

use std::sync::Arc;

use community_domain::{ModId, UserId};

use crate::ports::PortResult;
use crate::AppError;

/// Public numbers of one mod. Later issues add fields (for example downloads)
/// without touching the existing ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModStats {
    pub id: String,
    pub stars: u64,
    /// `Some` only when the caller is signed in.
    pub starred_by_me: Option<bool>,
}

#[async_trait::async_trait]
pub trait StarRepository: Send + Sync {
    /// Records the star; starring again changes nothing.
    async fn star(&self, user: &UserId, mod_id: &ModId) -> PortResult<()>;
    /// Removes the star; unstarring a mod that is not starred is a success.
    async fn unstar(&self, user: &UserId, mod_id: &ModId) -> PortResult<()>;
    async fn count(&self, mod_id: &ModId) -> PortResult<u64>;
    /// Every mod with at least one star, ordered by id. `starred_by_me` is
    /// filled only when a viewer is given.
    async fn stats(&self, viewer: Option<&UserId>) -> PortResult<Vec<ModStats>>;
}

pub struct StarService {
    pub stars: Arc<dyn StarRepository>,
}

impl StarService {
    pub fn new(stars: Arc<dyn StarRepository>) -> Self {
        Self { stars }
    }

    fn parse(id: &str) -> Result<ModId, AppError> {
        ModId::parse(id).ok_or_else(|| AppError::BadRequest("The mod id is invalid.".into()))
    }

    pub async fn star(&self, user: &UserId, id: &str) -> PortResult<ModStats> {
        let mod_id = Self::parse(id)?;
        self.stars.star(user, &mod_id).await?;
        self.after(&mod_id, true).await
    }

    pub async fn unstar(&self, user: &UserId, id: &str) -> PortResult<ModStats> {
        let mod_id = Self::parse(id)?;
        self.stars.unstar(user, &mod_id).await?;
        self.after(&mod_id, false).await
    }

    pub async fn stats(&self, viewer: Option<&UserId>) -> PortResult<Vec<ModStats>> {
        self.stars.stats(viewer).await
    }

    async fn after(&self, mod_id: &ModId, starred: bool) -> PortResult<ModStats> {
        Ok(ModStats {
            id: mod_id.to_string(),
            stars: self.stars.count(mod_id).await?,
            starred_by_me: Some(starred),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakeStars {
        rows: Mutex<BTreeSet<(String, String)>>,
        fail: Mutex<bool>,
    }

    impl FakeStars {
        fn check(&self) -> PortResult<()> {
            if *self.fail.lock().unwrap() {
                Err(AppError::Internal("down".into()))
            } else {
                Ok(())
            }
        }
    }

    #[async_trait::async_trait]
    impl StarRepository for FakeStars {
        async fn star(&self, user: &UserId, mod_id: &ModId) -> PortResult<()> {
            self.check()?;
            self.rows
                .lock()
                .unwrap()
                .insert((user.0.clone(), mod_id.to_string()));
            Ok(())
        }
        async fn unstar(&self, user: &UserId, mod_id: &ModId) -> PortResult<()> {
            self.check()?;
            self.rows
                .lock()
                .unwrap()
                .remove(&(user.0.clone(), mod_id.to_string()));
            Ok(())
        }
        async fn count(&self, mod_id: &ModId) -> PortResult<u64> {
            self.check()?;
            let rows = self.rows.lock().unwrap();
            Ok(rows.iter().filter(|(_, m)| m == mod_id.as_str()).count() as u64)
        }
        async fn stats(&self, viewer: Option<&UserId>) -> PortResult<Vec<ModStats>> {
            self.check()?;
            let rows = self.rows.lock().unwrap();
            let mods: BTreeSet<&String> = rows.iter().map(|(_, m)| m).collect();
            Ok(mods
                .into_iter()
                .map(|m| ModStats {
                    id: m.clone(),
                    stars: rows.iter().filter(|(_, x)| x == m).count() as u64,
                    starred_by_me: viewer.map(|v| rows.contains(&(v.0.clone(), m.clone()))),
                })
                .collect())
        }
    }

    fn service() -> (StarService, Arc<FakeStars>) {
        let fake = Arc::new(FakeStars::default());
        (StarService::new(fake.clone()), fake)
    }

    fn user(id: &str) -> UserId {
        UserId(id.into())
    }

    #[tokio::test]
    async fn starring_counts_and_marks_the_caller() {
        let (service, _) = service();
        service.star(&user("a"), "mod-1").await.unwrap();
        let stats = service.star(&user("b"), "mod-1").await.unwrap();
        assert_eq!(
            stats,
            ModStats {
                id: "mod-1".into(),
                stars: 2,
                starred_by_me: Some(true)
            }
        );
    }

    #[tokio::test]
    async fn starring_twice_counts_once() {
        let (service, _) = service();
        service.star(&user("a"), "mod-1").await.unwrap();
        let stats = service.star(&user("a"), "mod-1").await.unwrap();
        assert_eq!(stats.stars, 1);
    }

    #[tokio::test]
    async fn unstarring_removes_and_is_idempotent() {
        let (service, _) = service();
        service.star(&user("a"), "mod-1").await.unwrap();
        service.star(&user("b"), "mod-1").await.unwrap();
        let stats = service.unstar(&user("a"), "mod-1").await.unwrap();
        assert_eq!((stats.stars, stats.starred_by_me), (1, Some(false)));
        let again = service.unstar(&user("a"), "mod-1").await.unwrap();
        assert_eq!((again.stars, again.starred_by_me), (1, Some(false)));
        let never = service.unstar(&user("a"), "other").await.unwrap();
        assert_eq!(never.stars, 0);
    }

    #[tokio::test]
    async fn invalid_ids_are_bad_requests() {
        let (service, fake) = service();
        let error = service.star(&user("a"), "Not A Slug!").await.unwrap_err();
        assert_eq!(error.code(), "bad_request");
        let error = service.unstar(&user("a"), "").await.unwrap_err();
        assert_eq!(error.code(), "bad_request");
        assert!(fake.rows.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn stats_are_anonymous_or_personal() {
        let (service, _) = service();
        service.star(&user("a"), "b-mod").await.unwrap();
        service.star(&user("b"), "b-mod").await.unwrap();
        service.star(&user("b"), "a-mod").await.unwrap();

        let anonymous = service.stats(None).await.unwrap();
        assert_eq!(anonymous.len(), 2);
        assert_eq!(anonymous[0].id, "a-mod");
        assert!(anonymous.iter().all(|m| m.starred_by_me.is_none()));

        let mine = service.stats(Some(&user("a"))).await.unwrap();
        assert_eq!(mine[0].starred_by_me, Some(false));
        assert_eq!(mine[1].starred_by_me, Some(true));
        assert_eq!(mine[1].stars, 2);
    }

    #[tokio::test]
    async fn repository_failures_propagate() {
        let (service, fake) = service();
        *fake.fail.lock().unwrap() = true;
        assert!(service.star(&user("a"), "m").await.is_err());
        assert!(service.unstar(&user("a"), "m").await.is_err());
        assert!(service.stats(None).await.is_err());
    }
}
