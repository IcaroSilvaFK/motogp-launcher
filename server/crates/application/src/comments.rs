//! Mod comments: the repository port and the use cases (list, post, edit, delete).
//!
//! Not-yours is reported as `NotFound` (never a separate error), so a caller
//! cannot probe which comment ids exist.

use std::sync::Arc;

use community_domain::comments::{
    Comment, CommentBody, CommentId, ModId, PageCursor, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE,
};
use community_domain::{User, UserId};

use crate::ports::{Clock, PortResult, TokenGenerator};
use crate::AppError;

/// A comment together with its author's profile (the read model).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentWithAuthor {
    pub comment: Comment,
    pub author: User,
}

/// One page of comments, oldest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommentPage {
    pub comments: Vec<CommentWithAuthor>,
    /// Present when more comments follow; pass it back as `cursor`.
    pub next_cursor: Option<String>,
}

#[async_trait::async_trait]
pub trait CommentRepository: Send + Sync {
    async fn insert(&self, comment: &Comment) -> PortResult<()>;
    /// Up to `limit` comments of the mod ordered by `(created_at, id)` ascending,
    /// starting right after `after` when given.
    async fn page(
        &self,
        mod_id: &ModId,
        after: Option<&PageCursor>,
        limit: u32,
    ) -> PortResult<Vec<CommentWithAuthor>>;
    /// The comment when it exists under that mod.
    async fn find(&self, mod_id: &ModId, id: &CommentId) -> PortResult<Option<CommentWithAuthor>>;
    /// Replaces the body of a comment owned by `author`. `false` when there is
    /// no such comment of that author under that mod.
    async fn update_body(
        &self,
        mod_id: &ModId,
        id: &CommentId,
        author: &UserId,
        body: &CommentBody,
        edited_at: chrono::DateTime<chrono::Utc>,
    ) -> PortResult<bool>;
    /// Hard-deletes a comment owned by `author`. `false` when there is none.
    async fn delete(&self, mod_id: &ModId, id: &CommentId, author: &UserId) -> PortResult<bool>;
}

pub struct CommentsService {
    pub clock: Arc<dyn Clock>,
    pub tokens: Arc<dyn TokenGenerator>,
    pub comments: Arc<dyn CommentRepository>,
}

fn bad_request(message: &str) -> AppError {
    AppError::BadRequest(message.into())
}

fn parse_mod_id(value: &str) -> PortResult<ModId> {
    ModId::parse(value).ok_or_else(|| bad_request("The mod id is invalid."))
}

fn parse_body(value: &str) -> PortResult<CommentBody> {
    CommentBody::parse(value).map_err(|error| bad_request(error.message()))
}

impl CommentsService {
    /// Reads a page. `limit` is 1..=50 (default 20); a malformed cursor is a bad request.
    pub async fn list(
        &self,
        mod_id: &str,
        cursor: Option<&str>,
        limit: Option<u32>,
    ) -> PortResult<CommentPage> {
        let mod_id = parse_mod_id(mod_id)?;
        let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE);
        if !(1..=MAX_PAGE_SIZE).contains(&limit) {
            return Err(bad_request("The limit must be between 1 and 50."));
        }
        let after = cursor
            .map(|text| {
                PageCursor::decode(text).ok_or_else(|| bad_request("The cursor is invalid."))
            })
            .transpose()?;

        // One extra row tells whether another page follows.
        let mut rows = self
            .comments
            .page(&mod_id, after.as_ref(), limit + 1)
            .await?;
        let has_more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let next_cursor = match rows.last() {
            Some(last) if has_more => Some(
                PageCursor {
                    created_at_ms: last.comment.created_at.timestamp_millis(),
                    id: last.comment.id.0.clone(),
                }
                .encode(),
            ),
            _ => None,
        };
        Ok(CommentPage {
            comments: rows,
            next_cursor,
        })
    }

    pub async fn post(
        &self,
        author: &User,
        mod_id: &str,
        body: &str,
    ) -> PortResult<CommentWithAuthor> {
        let mod_id = parse_mod_id(mod_id)?;
        let body = parse_body(body)?;
        let comment = Comment {
            id: CommentId(self.tokens.generate(16)),
            mod_id,
            user_id: author.id.clone(),
            body,
            created_at: self.clock.now(),
            edited_at: None,
        };
        self.comments.insert(&comment).await?;
        Ok(CommentWithAuthor {
            comment,
            author: author.clone(),
        })
    }

    /// Edits the caller's own comment; any other case is `NotFound`.
    pub async fn edit(
        &self,
        author: &User,
        mod_id: &str,
        comment_id: &str,
        body: &str,
    ) -> PortResult<CommentWithAuthor> {
        let mod_id = parse_mod_id(mod_id)?;
        let body = parse_body(body)?;
        let id = CommentId::parse(comment_id).ok_or(AppError::NotFound)?;
        let updated = self
            .comments
            .update_body(&mod_id, &id, &author.id, &body, self.clock.now())
            .await?;
        if !updated {
            return Err(AppError::NotFound);
        }
        self.comments
            .find(&mod_id, &id)
            .await?
            .ok_or(AppError::NotFound)
    }

    /// Deletes the caller's own comment; any other case is `NotFound`.
    pub async fn delete(&self, author: &User, mod_id: &str, comment_id: &str) -> PortResult<()> {
        let mod_id = parse_mod_id(mod_id)?;
        let id = CommentId::parse(comment_id).ok_or(AppError::NotFound)?;
        let deleted = self.comments.delete(&mod_id, &id, &author.id).await?;
        deleted.then_some(()).ok_or(AppError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::{DateTime, Duration, Utc};
    use community_domain::DiscordId;

    use super::*;

    struct TestClock(Mutex<DateTime<Utc>>);

    impl Clock for TestClock {
        fn now(&self) -> DateTime<Utc> {
            *self.0.lock().unwrap()
        }
    }

    struct CountingTokens(Mutex<u32>);

    impl TokenGenerator for CountingTokens {
        fn generate(&self, _byte_len: usize) -> String {
            let mut n = self.0.lock().unwrap();
            *n += 1;
            format!("id{n:03}")
        }
    }

    /// In-memory repository with the same ordering and ownership rules as SQL.
    #[derive(Default)]
    struct MemoryRepo {
        rows: Mutex<Vec<CommentWithAuthor>>,
    }

    #[async_trait::async_trait]
    impl CommentRepository for MemoryRepo {
        async fn insert(&self, comment: &Comment) -> PortResult<()> {
            self.rows.lock().unwrap().push(CommentWithAuthor {
                comment: comment.clone(),
                author: user("author"),
            });
            Ok(())
        }

        async fn page(
            &self,
            mod_id: &ModId,
            after: Option<&PageCursor>,
            limit: u32,
        ) -> PortResult<Vec<CommentWithAuthor>> {
            let mut rows: Vec<_> = self
                .rows
                .lock()
                .unwrap()
                .iter()
                .filter(|r| &r.comment.mod_id == mod_id)
                .filter(|r| {
                    after.is_none_or(|c| {
                        (
                            r.comment.created_at.timestamp_millis(),
                            r.comment.id.0.clone(),
                        ) > (c.created_at_ms, c.id.clone())
                    })
                })
                .cloned()
                .collect();
            rows.sort_by_key(|r| (r.comment.created_at, r.comment.id.0.clone()));
            rows.truncate(limit as usize);
            Ok(rows)
        }

        async fn find(
            &self,
            mod_id: &ModId,
            id: &CommentId,
        ) -> PortResult<Option<CommentWithAuthor>> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .iter()
                .find(|r| &r.comment.mod_id == mod_id && &r.comment.id == id)
                .cloned())
        }

        async fn update_body(
            &self,
            mod_id: &ModId,
            id: &CommentId,
            author: &UserId,
            body: &CommentBody,
            edited_at: DateTime<Utc>,
        ) -> PortResult<bool> {
            let mut rows = self.rows.lock().unwrap();
            let found = rows.iter_mut().find(|r| {
                &r.comment.mod_id == mod_id && &r.comment.id == id && &r.comment.user_id == author
            });
            Ok(found
                .map(|r| {
                    r.comment.body = body.clone();
                    r.comment.edited_at = Some(edited_at);
                })
                .is_some())
        }

        async fn delete(
            &self,
            mod_id: &ModId,
            id: &CommentId,
            author: &UserId,
        ) -> PortResult<bool> {
            let mut rows = self.rows.lock().unwrap();
            let before = rows.len();
            rows.retain(|r| {
                !(&r.comment.mod_id == mod_id
                    && &r.comment.id == id
                    && &r.comment.user_id == author)
            });
            Ok(rows.len() < before)
        }
    }

    fn user(id: &str) -> User {
        User {
            id: UserId(id.into()),
            discord_id: DiscordId::parse("1").unwrap(),
            username: id.into(),
            global_name: None,
            avatar_hash: None,
            created_at: DateTime::from_timestamp(0, 0).unwrap(),
        }
    }

    fn service() -> (CommentsService, Arc<TestClock>) {
        let clock = Arc::new(TestClock(Mutex::new(
            DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        )));
        let service = CommentsService {
            clock: clock.clone(),
            tokens: Arc::new(CountingTokens(Mutex::new(0))),
            comments: Arc::new(MemoryRepo::default()),
        };
        (service, clock)
    }

    async fn post_n(service: &CommentsService, clock: &TestClock, author: &User, n: usize) {
        for i in 0..n {
            service
                .post(author, "mod-a", &format!("c{i}"))
                .await
                .unwrap();
            *clock.0.lock().unwrap() += Duration::seconds(1);
        }
    }

    fn bodies(page: &CommentPage) -> Vec<&str> {
        page.comments
            .iter()
            .map(|c| c.comment.body.as_str())
            .collect()
    }

    #[tokio::test]
    async fn post_stores_a_trimmed_body_for_the_author() {
        let (service, _) = service();
        let me = user("u1");
        let created = service
            .post(&me, "mod-a", "  Great livery \r\n")
            .await
            .unwrap();
        assert_eq!(created.comment.body.as_str(), "Great livery");
        assert_eq!(created.comment.user_id, me.id);
        assert_eq!(created.comment.edited_at, None);
        assert_eq!(created.author, me);
    }

    #[tokio::test]
    async fn post_rejects_bad_mod_ids_and_bodies() {
        let (service, _) = service();
        let me = user("u1");
        for (mod_id, body) in [
            ("Bad Mod", "x"),
            ("mod-a", ""),
            ("mod-a", &"a".repeat(2001)[..]),
        ] {
            let error = service.post(&me, mod_id, body).await.unwrap_err();
            assert!(
                matches!(error, AppError::BadRequest(_)),
                "{mod_id} {body:.5}"
            );
        }
        assert!(service
            .list("mod-a", None, None)
            .await
            .unwrap()
            .comments
            .is_empty());
    }

    #[tokio::test]
    async fn list_pages_oldest_first_with_a_cursor() {
        let (service, clock) = service();
        post_n(&service, &clock, &user("u1"), 25).await;

        let first = service.list("mod-a", None, None).await.unwrap();
        assert_eq!(first.comments.len(), 20);
        assert_eq!(bodies(&first)[0], "c0");
        assert_eq!(bodies(&first)[19], "c19");
        let cursor = first.next_cursor.expect("more pages follow");

        let second = service.list("mod-a", Some(&cursor), None).await.unwrap();
        assert_eq!(bodies(&second), ["c20", "c21", "c22", "c23", "c24"]);
        assert_eq!(second.next_cursor, None);
    }

    #[tokio::test]
    async fn list_has_no_cursor_when_the_page_is_exactly_full() {
        let (service, clock) = service();
        post_n(&service, &clock, &user("u1"), 3).await;
        let page = service.list("mod-a", None, Some(3)).await.unwrap();
        assert_eq!(page.comments.len(), 3);
        assert_eq!(page.next_cursor, None);
    }

    #[tokio::test]
    async fn list_validates_limit_cursor_and_mod_id() {
        let (service, _) = service();
        for limit in [0, 51] {
            let error = service.list("mod-a", None, Some(limit)).await.unwrap_err();
            assert!(matches!(error, AppError::BadRequest(_)));
        }
        assert!(service.list("mod-a", None, Some(50)).await.is_ok());
        assert!(matches!(
            service.list("mod-a", Some("garbage!"), None).await,
            Err(AppError::BadRequest(_))
        ));
        assert!(matches!(
            service.list("Mod A", None, None).await,
            Err(AppError::BadRequest(_))
        ));
    }

    #[tokio::test]
    async fn edit_changes_the_body_and_sets_edited_at() {
        let (service, clock) = service();
        let me = user("author");
        let created = service.post(&me, "mod-a", "Nice").await.unwrap();
        *clock.0.lock().unwrap() += Duration::minutes(5);

        let edited = service
            .edit(&me, "mod-a", created.comment.id.as_str(), "Nice livery")
            .await
            .unwrap();
        assert_eq!(edited.comment.body.as_str(), "Nice livery");
        assert_eq!(edited.comment.edited_at, Some(clock.now()));
        assert_eq!(edited.comment.created_at, created.comment.created_at);
    }

    #[tokio::test]
    async fn edit_and_delete_of_someone_elses_comment_are_not_found() {
        let (service, _) = service();
        let created = service
            .post(&user("author"), "mod-a", "Mine")
            .await
            .unwrap();
        let id = created.comment.id.as_str();
        let other = user("other");

        assert_eq!(
            service
                .edit(&other, "mod-a", id, "Hijacked")
                .await
                .unwrap_err(),
            AppError::NotFound
        );
        assert_eq!(
            service.delete(&other, "mod-a", id).await.unwrap_err(),
            AppError::NotFound
        );
        let page = service.list("mod-a", None, None).await.unwrap();
        assert_eq!(bodies(&page), ["Mine"]);
    }

    #[tokio::test]
    async fn edit_and_delete_under_another_mod_or_unknown_id_are_not_found() {
        let (service, _) = service();
        let me = user("author");
        let created = service.post(&me, "mod-a", "Mine").await.unwrap();
        let id = created.comment.id.as_str();

        for target in [("mod-b", id), ("mod-a", "nope"), ("mod-a", "bad id!")] {
            assert_eq!(
                service
                    .edit(&me, target.0, target.1, "x")
                    .await
                    .unwrap_err(),
                AppError::NotFound
            );
            assert_eq!(
                service.delete(&me, target.0, target.1).await.unwrap_err(),
                AppError::NotFound
            );
        }
    }

    #[tokio::test]
    async fn edit_validates_the_body_before_touching_anything() {
        let (service, _) = service();
        let me = user("author");
        let created = service.post(&me, "mod-a", "Keep").await.unwrap();
        let error = service
            .edit(&me, "mod-a", created.comment.id.as_str(), "  ")
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::BadRequest(_)));
        let page = service.list("mod-a", None, None).await.unwrap();
        assert_eq!(bodies(&page), ["Keep"]);
    }

    #[tokio::test]
    async fn delete_removes_the_comment_and_a_second_delete_is_not_found() {
        let (service, _) = service();
        let me = user("author");
        let created = service.post(&me, "mod-a", "Oops").await.unwrap();
        let id = created.comment.id.as_str();

        service.delete(&me, "mod-a", id).await.unwrap();
        assert!(service
            .list("mod-a", None, None)
            .await
            .unwrap()
            .comments
            .is_empty());
        assert_eq!(
            service.delete(&me, "mod-a", id).await.unwrap_err(),
            AppError::NotFound
        );
    }
}
