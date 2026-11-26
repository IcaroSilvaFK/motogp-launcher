//! SQLite adapter of the mod repository port.

use chrono::{DateTime, Utc};
use community_application::mod_ports::{ModRepository, NewVersion};
use community_application::ports::PortResult;
use community_application::AppError;
use community_domain::mods::{
    ModMetadata, ModSlug, ModUpload, ModVersion, PublishedMod, Screenshot, StorageKey,
    VersionStatus,
};
use community_domain::UserId;
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use crate::sqlite::SqliteStore;

const VERSION_COLUMNS: &str =
    "v.id, v.mod_id, m.owner_user_id, v.version, v.title, v.description, \
     v.long_description, v.category, v.game, v.file_name, v.size, v.sha256, v.storage_key, \
     v.status, v.rejection_reason, v.uploaded_at, v.reviewed_at";

fn internal(error: sqlx::Error) -> AppError {
    AppError::Internal(format!("database error: {error}"))
}

fn from_ts(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).unwrap_or_default()
}

fn bad_row(what: &str) -> AppError {
    AppError::Internal(format!("stored {what} is invalid"))
}

fn slug(value: &str) -> PortResult<ModSlug> {
    ModSlug::parse(value).ok_or_else(|| bad_row("mod id"))
}

fn version_from_row(row: &SqliteRow) -> PortResult<ModVersion> {
    let status: String = row.get("status");
    let size: i64 = row.get("size");
    let mod_id: String = row.get("mod_id");
    Ok(ModVersion {
        id: row.get("id"),
        mod_id: slug(&mod_id)?,
        owner_user_id: UserId(row.get("owner_user_id")),
        version: row.get("version"),
        title: row.get("title"),
        description: row.get("description"),
        long_description: row.get("long_description"),
        category: row.get("category"),
        game: row.get("game"),
        file_name: row.get("file_name"),
        size: u64::try_from(size).map_err(|_| bad_row("size"))?,
        sha256: row.get("sha256"),
        storage_key: StorageKey::from_stored(row.get("storage_key")),
        status: VersionStatus::parse(&status).ok_or_else(|| bad_row("status"))?,
        rejection_reason: row.get("rejection_reason"),
        uploaded_at: from_ts(row.get("uploaded_at")),
        reviewed_at: row.get::<Option<i64>, _>("reviewed_at").map(from_ts),
        screenshots: Vec::new(),
    })
}

fn upload_from_row(row: &SqliteRow) -> PortResult<ModUpload> {
    let mod_id: String = row.get("mod_id");
    let size: i64 = row.get("size");
    let received: i64 = row.get("received");
    Ok(ModUpload {
        id: row.get("id"),
        user_id: UserId(row.get("user_id")),
        meta: ModMetadata {
            mod_id: slug(&mod_id)?,
            title: row.get("title"),
            description: row.get("description"),
            long_description: row.get("long_description"),
            category: row.get("category"),
            game: row.get("game"),
            version: row.get("version"),
            file_name: row.get("file_name"),
            size: u64::try_from(size).map_err(|_| bad_row("size"))?,
            client_sha256: row.get("client_sha256"),
        },
        received: u64::try_from(received).map_err(|_| bad_row("received"))?,
        created_at: from_ts(row.get("created_at")),
        expires_at: from_ts(row.get("expires_at")),
    })
}

fn screenshot_from_row(row: &SqliteRow) -> Screenshot {
    let index: i64 = row.get("idx");
    Screenshot {
        index: usize::try_from(index).unwrap_or_default(),
        storage_key: StorageKey::from_stored(row.get("storage_key")),
        content_type: row.get("content_type"),
    }
}

fn to_i64(value: u64) -> PortResult<i64> {
    i64::try_from(value).map_err(|_| AppError::BadRequest("The number is too large.".into()))
}

impl SqliteStore {
    async fn attach_screenshots(&self, versions: &mut [ModVersion]) -> PortResult<()> {
        for version in versions {
            let rows = sqlx::query(
                "SELECT idx, storage_key, content_type FROM mod_screenshots
                 WHERE version_id = ?1 ORDER BY idx",
            )
            .bind(&version.id)
            .fetch_all(self.pool())
            .await
            .map_err(internal)?;
            version.screenshots = rows.iter().map(screenshot_from_row).collect();
        }
        Ok(())
    }

    async fn versions_where(
        &self,
        clause: &str,
        bind: Option<&str>,
    ) -> PortResult<Vec<ModVersion>> {
        let sql = format!(
            "SELECT {VERSION_COLUMNS} FROM mod_versions v JOIN mods m ON m.id = v.mod_id
             WHERE {clause} ORDER BY v.mod_id, v.uploaded_at, v.rowid"
        );
        let mut query = sqlx::query(&sql);
        if let Some(value) = bind {
            query = query.bind(value);
        }
        let rows = query.fetch_all(self.pool()).await.map_err(internal)?;
        let mut versions = rows
            .iter()
            .map(version_from_row)
            .collect::<PortResult<Vec<_>>>()?;
        self.attach_screenshots(&mut versions).await?;
        Ok(versions)
    }
}

#[async_trait::async_trait]
impl ModRepository for SqliteStore {
    async fn mod_owner(&self, mod_id: &ModSlug) -> PortResult<Option<UserId>> {
        let owner: Option<String> =
            sqlx::query_scalar("SELECT owner_user_id FROM mods WHERE id = ?1")
                .bind(mod_id.as_str())
                .fetch_optional(self.pool())
                .await
                .map_err(internal)?;
        Ok(owner.map(UserId))
    }

    async fn find_version(
        &self,
        mod_id: &ModSlug,
        version: &str,
    ) -> PortResult<Option<ModVersion>> {
        let sql = format!(
            "SELECT {VERSION_COLUMNS} FROM mod_versions v JOIN mods m ON m.id = v.mod_id
             WHERE v.mod_id = ?1 AND v.version = ?2"
        );
        let row = sqlx::query(&sql)
            .bind(mod_id.as_str())
            .bind(version)
            .fetch_optional(self.pool())
            .await
            .map_err(internal)?;
        let Some(row) = row else { return Ok(None) };
        let mut versions = vec![version_from_row(&row)?];
        self.attach_screenshots(&mut versions).await?;
        Ok(versions.pop())
    }

    async fn create_upload(&self, upload: &ModUpload) -> PortResult<()> {
        let mut tx = self.pool().begin().await.map_err(internal)?;
        sqlx::query(
            "INSERT INTO mod_uploads (id, user_id, mod_id, version, title, description,
                 long_description, category, game, file_name, size, client_sha256, received,
                 created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        )
        .bind(&upload.id)
        .bind(upload.user_id.as_str())
        .bind(upload.meta.mod_id.as_str())
        .bind(&upload.meta.version)
        .bind(&upload.meta.title)
        .bind(&upload.meta.description)
        .bind(&upload.meta.long_description)
        .bind(&upload.meta.category)
        .bind(&upload.meta.game)
        .bind(&upload.meta.file_name)
        .bind(to_i64(upload.meta.size)?)
        .bind(&upload.meta.client_sha256)
        .bind(to_i64(upload.received)?)
        .bind(upload.created_at.timestamp())
        .bind(upload.expires_at.timestamp())
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        sqlx::query("INSERT INTO mod_upload_starts (user_id, started_at) VALUES (?1, ?2)")
            .bind(upload.user_id.as_str())
            .bind(upload.created_at.timestamp())
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        tx.commit().await.map_err(internal)
    }

    async fn get_upload(&self, id: &str) -> PortResult<Option<ModUpload>> {
        let row = sqlx::query("SELECT * FROM mod_uploads WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.pool())
            .await
            .map_err(internal)?;
        row.as_ref().map(upload_from_row).transpose()
    }

    async fn set_received(&self, id: &str, received: u64) -> PortResult<()> {
        sqlx::query("UPDATE mod_uploads SET received = ?2 WHERE id = ?1")
            .bind(id)
            .bind(to_i64(received)?)
            .execute(self.pool())
            .await
            .map(|_| ())
            .map_err(internal)
    }

    async fn delete_upload(&self, id: &str) -> PortResult<()> {
        sqlx::query("DELETE FROM mod_uploads WHERE id = ?1")
            .bind(id)
            .execute(self.pool())
            .await
            .map(|_| ())
            .map_err(internal)
    }

    async fn expired_uploads(&self, now: DateTime<Utc>) -> PortResult<Vec<ModUpload>> {
        let rows = sqlx::query("SELECT * FROM mod_uploads WHERE expires_at <= ?1")
            .bind(now.timestamp())
            .fetch_all(self.pool())
            .await
            .map_err(internal)?;
        rows.iter().map(upload_from_row).collect()
    }

    async fn upload_starts_since(
        &self,
        user: &UserId,
        since: DateTime<Utc>,
    ) -> PortResult<Vec<DateTime<Utc>>> {
        let rows: Vec<i64> = sqlx::query_scalar(
            "SELECT started_at FROM mod_upload_starts WHERE user_id = ?1 AND started_at > ?2",
        )
        .bind(user.as_str())
        .bind(since.timestamp())
        .fetch_all(self.pool())
        .await
        .map_err(internal)?;
        Ok(rows.into_iter().map(from_ts).collect())
    }

    async fn stored_bytes(&self, user: &UserId) -> PortResult<u64> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE((SELECT SUM(v.size) FROM mod_versions v
                              JOIN mods m ON m.id = v.mod_id
                              WHERE m.owner_user_id = ?1 AND v.status != 'rejected'), 0)
                  + COALESCE((SELECT SUM(size) FROM mod_uploads WHERE user_id = ?1), 0)",
        )
        .bind(user.as_str())
        .fetch_one(self.pool())
        .await
        .map_err(internal)?;
        Ok(u64::try_from(total).unwrap_or_default())
    }

    async fn add_upload_screenshot(
        &self,
        upload_id: &str,
        screenshot: &Screenshot,
    ) -> PortResult<()> {
        sqlx::query(
            "INSERT INTO mod_screenshots (upload_id, idx, storage_key, content_type)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(upload_id)
        .bind(i64::try_from(screenshot.index).unwrap_or(i64::MAX))
        .bind(screenshot.storage_key.as_str())
        .bind(&screenshot.content_type)
        .execute(self.pool())
        .await
        .map(|_| ())
        .map_err(|error| match &error {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                AppError::BadRequest("That screenshot slot is already taken.".into())
            }
            _ => internal(error),
        })
    }

    async fn upload_screenshots(&self, upload_id: &str) -> PortResult<Vec<Screenshot>> {
        let rows = sqlx::query(
            "SELECT idx, storage_key, content_type FROM mod_screenshots
             WHERE upload_id = ?1 ORDER BY idx",
        )
        .bind(upload_id)
        .fetch_all(self.pool())
        .await
        .map_err(internal)?;
        Ok(rows.iter().map(screenshot_from_row).collect())
    }

    async fn complete_upload(&self, upload_id: &str, new: &NewVersion) -> PortResult<ModVersion> {
        let mut tx = self.pool().begin().await.map_err(internal)?;
        let mod_id = new.meta.mod_id.as_str();

        let owner: Option<String> =
            sqlx::query_scalar("SELECT owner_user_id FROM mods WHERE id = ?1")
                .bind(mod_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(internal)?;
        match owner {
            Some(owner) if owner != new.owner.as_str() => return Err(AppError::ModIdTaken),
            Some(_) => {}
            None => {
                sqlx::query("INSERT INTO mods (id, owner_user_id, created_at) VALUES (?1, ?2, ?3)")
                    .bind(mod_id)
                    .bind(new.owner.as_str())
                    .bind(new.uploaded_at.timestamp())
                    .execute(&mut *tx)
                    .await
                    .map_err(internal)?;
            }
        }

        let existing: Option<String> = sqlx::query_scalar(
            "SELECT status FROM mod_versions WHERE mod_id = ?1 AND version = ?2",
        )
        .bind(mod_id)
        .bind(&new.meta.version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(internal)?;
        match existing.as_deref() {
            Some("rejected") => {
                sqlx::query("DELETE FROM mod_versions WHERE mod_id = ?1 AND version = ?2")
                    .bind(mod_id)
                    .bind(&new.meta.version)
                    .execute(&mut *tx)
                    .await
                    .map_err(internal)?;
            }
            Some(_) => return Err(AppError::VersionExists),
            None => {}
        }

        sqlx::query(
            "INSERT INTO mod_versions (id, mod_id, version, title, description, long_description,
                 category, game, file_name, size, sha256, storage_key, status,
                 ownership_confirmed, uploaded_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'pending', 1, ?13)",
        )
        .bind(&new.id)
        .bind(mod_id)
        .bind(&new.meta.version)
        .bind(&new.meta.title)
        .bind(&new.meta.description)
        .bind(&new.meta.long_description)
        .bind(&new.meta.category)
        .bind(&new.meta.game)
        .bind(&new.meta.file_name)
        .bind(to_i64(new.meta.size)?)
        .bind(&new.sha256)
        .bind(new.storage_key.as_str())
        .bind(new.uploaded_at.timestamp())
        .execute(&mut *tx)
        .await
        .map_err(|error| match &error {
            sqlx::Error::Database(db) if db.is_unique_violation() => AppError::VersionExists,
            _ => internal(error),
        })?;

        // Detach the screenshots before the session row goes away (it cascades).
        sqlx::query(
            "UPDATE mod_screenshots SET version_id = ?1, upload_id = NULL WHERE upload_id = ?2",
        )
        .bind(&new.id)
        .bind(upload_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        sqlx::query("DELETE FROM mod_uploads WHERE id = ?1")
            .bind(upload_id)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        tx.commit().await.map_err(internal)?;

        self.find_version(&new.meta.mod_id, &new.meta.version)
            .await?
            .ok_or_else(|| AppError::Internal("the new version vanished".into()))
    }

    async fn versions_of_user(&self, user: &UserId) -> PortResult<Vec<ModVersion>> {
        self.versions_where("m.owner_user_id = ?1", Some(user.as_str()))
            .await
    }

    async fn pending_versions(&self) -> PortResult<Vec<ModVersion>> {
        self.versions_where("v.status = 'pending'", None).await
    }

    async fn review(
        &self,
        version_id: &str,
        status: VersionStatus,
        reason: Option<&str>,
        reviewer: &UserId,
        now: DateTime<Utc>,
    ) -> PortResult<()> {
        let mut tx = self.pool().begin().await.map_err(internal)?;
        sqlx::query(
            "UPDATE mod_versions SET status = ?2, rejection_reason = ?3, reviewed_at = ?4,
                 reviewed_by = ?5 WHERE id = ?1",
        )
        .bind(version_id)
        .bind(status.as_str())
        .bind(reason)
        .bind(now.timestamp())
        .bind(reviewer.as_str())
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        if status == VersionStatus::Rejected {
            sqlx::query("DELETE FROM mod_screenshots WHERE version_id = ?1")
                .bind(version_id)
                .execute(&mut *tx)
                .await
                .map_err(internal)?;
        }
        tx.commit().await.map_err(internal)
    }

    async fn latest_approved(&self) -> PortResult<Vec<PublishedMod>> {
        let sql = format!(
            "SELECT {VERSION_COLUMNS}, COALESCE(NULLIF(u.global_name, ''), u.username) AS author
             FROM mod_versions v
             JOIN mods m ON m.id = v.mod_id
             JOIN users u ON u.id = m.owner_user_id
             WHERE v.status = 'approved'
               AND v.id = (SELECT v2.id FROM mod_versions v2
                           WHERE v2.mod_id = v.mod_id AND v2.status = 'approved'
                           ORDER BY v2.reviewed_at DESC, v2.rowid DESC LIMIT 1)
             ORDER BY v.mod_id"
        );
        let rows = sqlx::query(&sql)
            .fetch_all(self.pool())
            .await
            .map_err(internal)?;
        let mut published = Vec::with_capacity(rows.len());
        for row in &rows {
            published.push(PublishedMod {
                version: version_from_row(row)?,
                author: row.get("author"),
            });
        }
        let mut versions: Vec<ModVersion> = published.iter().map(|p| p.version.clone()).collect();
        self.attach_screenshots(&mut versions).await?;
        for (entry, version) in published.iter_mut().zip(versions) {
            entry.version = version;
        }
        Ok(published)
    }
}

#[cfg(test)]
mod tests {
    use community_application::ports::{UserRepository, UserUpsert};
    use community_domain::mods::RawMetadata;
    use community_domain::DiscordId;

    use super::*;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).unwrap()
    }

    async fn store_with_users() -> SqliteStore {
        let store = SqliteStore::connect_in_memory().await.unwrap();
        store.migrate().await.unwrap();
        for (id, discord, name) in [("u1", "111", "alice"), ("u2", "222", "bob")] {
            store
                .upsert(&UserUpsert {
                    new_id: UserId(id.into()),
                    discord_id: DiscordId::parse(discord).unwrap(),
                    username: name.into(),
                    global_name: (id == "u1").then(|| "Alice Rider".to_owned()),
                    avatar_hash: None,
                    now: at(1),
                })
                .await
                .unwrap();
        }
        store
    }

    fn meta(mod_id: &str, version: &str, size: u64) -> ModMetadata {
        ModMetadata::validate(RawMetadata {
            mod_id: mod_id.into(),
            title: "Title".into(),
            description: "Desc".into(),
            long_description: Some("Long".into()),
            category: "liveries".into(),
            game: "MotoGP 26".into(),
            version: version.into(),
            file_name: "a.zip".into(),
            size,
            sha256: Some("b".repeat(64)),
            ownership_confirmed: true,
        })
        .unwrap()
    }

    fn upload(id: &str, user: &str, meta: ModMetadata, created: i64) -> ModUpload {
        ModUpload {
            id: id.into(),
            user_id: UserId(user.into()),
            meta,
            received: 0,
            created_at: at(created),
            expires_at: at(created + 86_400),
        }
    }

    fn new_version(id: &str, user: &str, meta: ModMetadata, uploaded: i64) -> NewVersion {
        NewVersion {
            id: id.into(),
            owner: UserId(user.into()),
            storage_key: StorageKey::file(&meta.mod_id, &meta.version, &meta.file_name),
            sha256: "c".repeat(64),
            meta,
            uploaded_at: at(uploaded),
        }
    }

    async fn publish(
        store: &SqliteStore,
        upload_id: &str,
        user: &str,
        mod_id: &str,
        version: &str,
    ) {
        let m = meta(mod_id, version, 10);
        store
            .create_upload(&upload(upload_id, user, m.clone(), 100))
            .await
            .unwrap();
        store
            .complete_upload(
                upload_id,
                &new_version(&format!("v-{upload_id}"), user, m, 200),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn upload_sessions_round_trip_and_track_progress() {
        let store = store_with_users().await;
        let m = meta("brazil-livery", "1.0", 10);
        store
            .create_upload(&upload("up1", "u1", m.clone(), 100))
            .await
            .unwrap();
        let loaded = store.get_upload("up1").await.unwrap().unwrap();
        assert_eq!(loaded.meta, m);
        assert_eq!(loaded.received, 0);
        store.set_received("up1", 4).await.unwrap();
        assert_eq!(store.get_upload("up1").await.unwrap().unwrap().received, 4);
        assert!(store.get_upload("nope").await.unwrap().is_none());

        assert!(store.expired_uploads(at(100)).await.unwrap().is_empty());
        assert_eq!(
            store.expired_uploads(at(100 + 86_400)).await.unwrap().len(),
            1
        );
        store.delete_upload("up1").await.unwrap();
        assert!(store.get_upload("up1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn starts_are_counted_even_after_the_session_is_gone() {
        let store = store_with_users().await;
        let m = meta("brazil-livery", "1.0", 10);
        store
            .create_upload(&upload("up1", "u1", m.clone(), 100))
            .await
            .unwrap();
        store
            .create_upload(&upload("up2", "u1", m, 300))
            .await
            .unwrap();
        store.delete_upload("up1").await.unwrap();
        let u1 = UserId("u1".into());
        assert_eq!(
            store.upload_starts_since(&u1, at(0)).await.unwrap().len(),
            2
        );
        assert_eq!(
            store.upload_starts_since(&u1, at(100)).await.unwrap(),
            vec![at(300)]
        );
        assert!(store
            .upload_starts_since(&UserId("u2".into()), at(0))
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn stored_bytes_count_open_uploads_and_non_rejected_versions() {
        let store = store_with_users().await;
        let u1 = UserId("u1".into());
        assert_eq!(store.stored_bytes(&u1).await.unwrap(), 0);
        publish(&store, "up1", "u1", "brazil-livery", "1.0").await;
        assert_eq!(store.stored_bytes(&u1).await.unwrap(), 10);
        store
            .create_upload(&upload("up2", "u1", meta("brazil-livery", "1.1", 25), 300))
            .await
            .unwrap();
        assert_eq!(store.stored_bytes(&u1).await.unwrap(), 35);
        store
            .review(
                "v-up1",
                VersionStatus::Rejected,
                Some("no"),
                &UserId("u2".into()),
                at(400),
            )
            .await
            .unwrap();
        assert_eq!(store.stored_bytes(&u1).await.unwrap(), 25);
    }

    #[tokio::test]
    async fn complete_creates_the_mod_and_moves_the_screenshots() {
        let store = store_with_users().await;
        let m = meta("brazil-livery", "1.0", 10);
        store
            .create_upload(&upload("up1", "u1", m.clone(), 100))
            .await
            .unwrap();
        let shot = Screenshot {
            index: 0,
            storage_key: StorageKey::from_stored("brazil-livery/1.0/screenshot-0.png".into()),
            content_type: "image/png".into(),
        };
        store.add_upload_screenshot("up1", &shot).await.unwrap();
        assert_eq!(
            store.upload_screenshots("up1").await.unwrap(),
            vec![shot.clone()]
        );
        assert!(matches!(
            store.add_upload_screenshot("up1", &shot).await,
            Err(AppError::BadRequest(_))
        ));

        let version = store
            .complete_upload("up1", &new_version("v1", "u1", m.clone(), 200))
            .await
            .unwrap();
        assert_eq!(version.status, VersionStatus::Pending);
        assert_eq!(version.screenshots, vec![shot]);
        assert_eq!(version.long_description.as_deref(), Some("Long"));
        assert!(store.get_upload("up1").await.unwrap().is_none());
        assert_eq!(
            store.mod_owner(&m.mod_id).await.unwrap(),
            Some(UserId("u1".into()))
        );
        assert!(store
            .find_version(&m.mod_id, "9.9")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn complete_refuses_foreign_mods_and_duplicate_versions() {
        let store = store_with_users().await;
        publish(&store, "up1", "u1", "brazil-livery", "1.0").await;

        let m = meta("brazil-livery", "2.0", 10);
        store
            .create_upload(&upload("up2", "u2", m.clone(), 300))
            .await
            .unwrap();
        assert_eq!(
            store
                .complete_upload("up2", &new_version("v2", "u2", m, 400))
                .await
                .unwrap_err(),
            AppError::ModIdTaken
        );

        let dup = meta("brazil-livery", "1.0", 10);
        store
            .create_upload(&upload("up3", "u1", dup.clone(), 300))
            .await
            .unwrap();
        assert_eq!(
            store
                .complete_upload("up3", &new_version("v3", "u1", dup, 400))
                .await
                .unwrap_err(),
            AppError::VersionExists
        );
        // The failed attempts left the session (and nothing else) behind.
        assert!(store.get_upload("up3").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn a_rejected_version_can_be_replaced() {
        let store = store_with_users().await;
        publish(&store, "up1", "u1", "brazil-livery", "1.0").await;
        store
            .review(
                "v-up1",
                VersionStatus::Rejected,
                Some("bad"),
                &UserId("u2".into()),
                at(250),
            )
            .await
            .unwrap();
        publish(&store, "up2", "u1", "brazil-livery", "1.0").await;
        let versions = store.versions_of_user(&UserId("u1".into())).await.unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].status, VersionStatus::Pending);
    }

    #[tokio::test]
    async fn review_sets_status_reason_and_drops_screenshots_on_reject() {
        let store = store_with_users().await;
        let m = meta("brazil-livery", "1.0", 10);
        store
            .create_upload(&upload("up1", "u1", m.clone(), 100))
            .await
            .unwrap();
        store
            .add_upload_screenshot(
                "up1",
                &Screenshot {
                    index: 0,
                    storage_key: StorageKey::from_stored(
                        "brazil-livery/1.0/screenshot-0.png".into(),
                    ),
                    content_type: "image/png".into(),
                },
            )
            .await
            .unwrap();
        store
            .complete_upload("up1", &new_version("v1", "u1", m.clone(), 200))
            .await
            .unwrap();
        assert_eq!(store.pending_versions().await.unwrap().len(), 1);

        let admin = UserId("u2".into());
        store
            .review(
                "v1",
                VersionStatus::Rejected,
                Some("Not allowed"),
                &admin,
                at(300),
            )
            .await
            .unwrap();
        let version = store.find_version(&m.mod_id, "1.0").await.unwrap().unwrap();
        assert_eq!(version.status, VersionStatus::Rejected);
        assert_eq!(version.rejection_reason.as_deref(), Some("Not allowed"));
        assert_eq!(version.reviewed_at, Some(at(300)));
        assert!(version.screenshots.is_empty());
        assert!(store.pending_versions().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_catalog_has_the_latest_approved_version_per_mod_with_the_author() {
        let store = store_with_users().await;
        let admin = UserId("u2".into());
        publish(&store, "up1", "u1", "brazil-livery", "1.0").await;
        publish(&store, "up2", "u1", "brazil-livery", "1.1").await;
        publish(&store, "up3", "u2", "other-mod", "0.1").await;
        publish(&store, "up4", "u2", "waiting-mod", "0.1").await;
        store
            .review("v-up1", VersionStatus::Approved, None, &admin, at(300))
            .await
            .unwrap();
        store
            .review("v-up2", VersionStatus::Approved, None, &admin, at(400))
            .await
            .unwrap();
        store
            .review("v-up3", VersionStatus::Approved, None, &admin, at(350))
            .await
            .unwrap();

        let catalog = store.latest_approved().await.unwrap();
        let summary: Vec<_> = catalog
            .iter()
            .map(|p| {
                (
                    p.version.mod_id.as_str(),
                    p.version.version.as_str(),
                    p.author.as_str(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("brazil-livery", "1.1", "Alice Rider"),
                ("other-mod", "0.1", "bob")
            ]
        );

        // History stays: the owner sees both versions.
        let mine = store.versions_of_user(&UserId("u1".into())).await.unwrap();
        assert_eq!(mine.len(), 2);
    }

    #[tokio::test]
    async fn out_of_range_numbers_are_refused() {
        assert!(to_i64(u64::MAX).is_err());
        assert_eq!(to_i64(5).unwrap(), 5);
    }
}
