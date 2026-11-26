//! Use-case tests over in-memory fakes of the storage and repository ports.

use std::collections::HashMap;
use std::sync::Mutex;

use community_domain::UserId;

use super::*;

struct FakeClock(Mutex<DateTime<Utc>>);

impl FakeClock {
    fn advance(&self, by: Duration) {
        *self.0.lock().unwrap() += by;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}

struct CountingTokens(Mutex<u32>);

impl TokenGenerator for CountingTokens {
    fn generate(&self, _byte_len: usize) -> String {
        let mut n = self.0.lock().unwrap();
        *n += 1;
        format!("tok{n}")
    }
}

#[derive(Default)]
struct FakeStorage {
    incoming: Mutex<HashMap<String, Vec<u8>>>,
    pending: Mutex<HashMap<String, Vec<u8>>>,
    public: Mutex<HashMap<String, Vec<u8>>>,
}

#[async_trait::async_trait]
impl ModStorage for FakeStorage {
    async fn append_incoming(&self, id: &str, offset: u64, bytes: &[u8]) -> PortResult<()> {
        let mut map = self.incoming.lock().unwrap();
        let file = map.entry(id.to_owned()).or_default();
        file.truncate(offset as usize);
        file.extend_from_slice(bytes);
        Ok(())
    }
    async fn incoming_len(&self, id: &str) -> PortResult<u64> {
        Ok(self
            .incoming
            .lock()
            .unwrap()
            .get(id)
            .map_or(0, |f| f.len() as u64))
    }
    async fn sha256_incoming(&self, id: &str) -> PortResult<String> {
        // Stand-in digest: the real adapter computes SHA-256.
        let map = self.incoming.lock().unwrap();
        Ok(format!("sum:{}", map.get(id).map_or(0, Vec::len)))
    }
    async fn commit_incoming(&self, id: &str, key: &StorageKey) -> PortResult<()> {
        let bytes = self
            .incoming
            .lock()
            .unwrap()
            .remove(id)
            .ok_or(AppError::NotFound)?;
        self.pending
            .lock()
            .unwrap()
            .insert(key.as_str().to_owned(), bytes);
        Ok(())
    }
    async fn put_pending(&self, key: &StorageKey, bytes: &[u8]) -> PortResult<()> {
        self.pending
            .lock()
            .unwrap()
            .insert(key.as_str().to_owned(), bytes.to_vec());
        Ok(())
    }
    async fn publish(&self, key: &StorageKey) -> PortResult<()> {
        if let Some(bytes) = self.pending.lock().unwrap().remove(key.as_str()) {
            self.public
                .lock()
                .unwrap()
                .insert(key.as_str().to_owned(), bytes);
        }
        Ok(())
    }
    async fn delete(&self, key: &StorageKey) -> PortResult<()> {
        self.pending.lock().unwrap().remove(key.as_str());
        self.public.lock().unwrap().remove(key.as_str());
        Ok(())
    }
    async fn delete_incoming(&self, id: &str) -> PortResult<()> {
        self.incoming.lock().unwrap().remove(id);
        Ok(())
    }
}

#[derive(Default)]
struct RepoState {
    owners: HashMap<String, UserId>,
    versions: Vec<ModVersion>,
    uploads: Vec<ModUpload>,
    starts: Vec<(UserId, DateTime<Utc>)>,
    screenshots: HashMap<String, Vec<Screenshot>>,
}

#[derive(Default)]
struct FakeRepo(Mutex<RepoState>);

#[async_trait::async_trait]
impl ModRepository for FakeRepo {
    async fn mod_owner(&self, mod_id: &ModSlug) -> PortResult<Option<UserId>> {
        Ok(self.0.lock().unwrap().owners.get(mod_id.as_str()).cloned())
    }
    async fn find_version(
        &self,
        mod_id: &ModSlug,
        version: &str,
    ) -> PortResult<Option<ModVersion>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .versions
            .iter()
            .find(|v| v.mod_id == *mod_id && v.version == version)
            .cloned())
    }
    async fn create_upload(&self, upload: &ModUpload) -> PortResult<()> {
        let mut state = self.0.lock().unwrap();
        state.uploads.push(upload.clone());
        state
            .starts
            .push((upload.user_id.clone(), upload.created_at));
        Ok(())
    }
    async fn get_upload(&self, id: &str) -> PortResult<Option<ModUpload>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .uploads
            .iter()
            .find(|u| u.id == id)
            .cloned())
    }
    async fn set_received(&self, id: &str, received: u64) -> PortResult<()> {
        let mut state = self.0.lock().unwrap();
        if let Some(u) = state.uploads.iter_mut().find(|u| u.id == id) {
            u.received = received;
        }
        Ok(())
    }
    async fn delete_upload(&self, id: &str) -> PortResult<()> {
        let mut state = self.0.lock().unwrap();
        state.uploads.retain(|u| u.id != id);
        state.screenshots.remove(id);
        Ok(())
    }
    async fn expired_uploads(&self, now: DateTime<Utc>) -> PortResult<Vec<ModUpload>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .uploads
            .iter()
            .filter(|u| u.expires_at <= now)
            .cloned()
            .collect())
    }
    async fn upload_starts_since(
        &self,
        user: &UserId,
        since: DateTime<Utc>,
    ) -> PortResult<Vec<DateTime<Utc>>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .starts
            .iter()
            .filter(|(u, at)| u == user && *at > since)
            .map(|(_, at)| *at)
            .collect())
    }
    async fn stored_bytes(&self, user: &UserId) -> PortResult<u64> {
        let state = self.0.lock().unwrap();
        let versions: u64 = state
            .versions
            .iter()
            .filter(|v| v.owner_user_id == *user && v.status != VersionStatus::Rejected)
            .map(|v| v.size)
            .sum();
        let open: u64 = state
            .uploads
            .iter()
            .filter(|u| u.user_id == *user)
            .map(|u| u.meta.size)
            .sum();
        Ok(versions + open)
    }
    async fn add_upload_screenshot(&self, id: &str, shot: &Screenshot) -> PortResult<()> {
        self.0
            .lock()
            .unwrap()
            .screenshots
            .entry(id.to_owned())
            .or_default()
            .push(shot.clone());
        Ok(())
    }
    async fn upload_screenshots(&self, id: &str) -> PortResult<Vec<Screenshot>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .screenshots
            .get(id)
            .cloned()
            .unwrap_or_default())
    }
    async fn complete_upload(&self, id: &str, new: &NewVersion) -> PortResult<ModVersion> {
        let mut state = self.0.lock().unwrap();
        let screenshots = state.screenshots.remove(id).unwrap_or_default();
        state.uploads.retain(|u| u.id != id);
        state
            .owners
            .entry(new.meta.mod_id.as_str().to_owned())
            .or_insert_with(|| new.owner.clone());
        state
            .versions
            .retain(|v| !(v.mod_id == new.meta.mod_id && v.version == new.meta.version));
        let version = ModVersion {
            id: new.id.clone(),
            mod_id: new.meta.mod_id.clone(),
            owner_user_id: new.owner.clone(),
            version: new.meta.version.clone(),
            title: new.meta.title.clone(),
            description: new.meta.description.clone(),
            long_description: new.meta.long_description.clone(),
            category: new.meta.category.clone(),
            game: new.meta.game.clone(),
            file_name: new.meta.file_name.clone(),
            size: new.meta.size,
            sha256: new.sha256.clone(),
            storage_key: new.storage_key.clone(),
            status: VersionStatus::Pending,
            rejection_reason: None,
            uploaded_at: new.uploaded_at,
            reviewed_at: None,
            screenshots,
        };
        state.versions.push(version.clone());
        Ok(version)
    }
    async fn versions_of_user(&self, user: &UserId) -> PortResult<Vec<ModVersion>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .versions
            .iter()
            .filter(|v| v.owner_user_id == *user)
            .cloned()
            .collect())
    }
    async fn pending_versions(&self) -> PortResult<Vec<ModVersion>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .versions
            .iter()
            .filter(|v| v.status == VersionStatus::Pending)
            .cloned()
            .collect())
    }
    async fn review(
        &self,
        id: &str,
        status: VersionStatus,
        reason: Option<&str>,
        _reviewer: &UserId,
        now: DateTime<Utc>,
    ) -> PortResult<()> {
        let mut state = self.0.lock().unwrap();
        if let Some(v) = state.versions.iter_mut().find(|v| v.id == id) {
            v.status = status;
            v.rejection_reason = reason.map(str::to_owned);
            v.reviewed_at = Some(now);
        }
        Ok(())
    }
    async fn latest_approved(&self) -> PortResult<Vec<PublishedMod>> {
        Ok(self
            .0
            .lock()
            .unwrap()
            .versions
            .iter()
            .filter(|v| v.status == VersionStatus::Approved)
            .map(|v| PublishedMod {
                version: v.clone(),
                author: "author".into(),
            })
            .collect())
    }
}

struct Fixture {
    service: ModHostingService,
    clock: Arc<FakeClock>,
    storage: Arc<FakeStorage>,
    repo: Arc<FakeRepo>,
}

fn user(id: &str, discord: &str) -> User {
    User {
        id: UserId(id.into()),
        discord_id: DiscordId::parse(discord).unwrap(),
        username: id.into(),
        global_name: None,
        avatar_hash: None,
        created_at: DateTime::from_timestamp(0, 0).unwrap(),
    }
}

fn creator() -> User {
    user("creator", "111")
}

fn admin() -> User {
    user("admin", "999")
}

fn fixture_with(limits: ModLimits) -> Fixture {
    let clock = Arc::new(FakeClock(Mutex::new(
        DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
    )));
    let storage = Arc::new(FakeStorage::default());
    let repo = Arc::new(FakeRepo::default());
    Fixture {
        service: ModHostingService {
            clock: clock.clone(),
            tokens: Arc::new(CountingTokens(Mutex::new(0))),
            repo: repo.clone(),
            storage: storage.clone(),
            admin_discord_ids: vec![DiscordId::parse("999").unwrap()],
            limits,
        },
        clock,
        storage,
        repo,
    }
}

fn fixture() -> Fixture {
    fixture_with(ModLimits {
        max_upload_bytes: 1_000,
        user_quota_bytes: 5_000,
        uploads_per_day: 3,
    })
}

fn raw(version: &str, size: u64) -> RawMetadata {
    RawMetadata {
        mod_id: "brazil-livery".into(),
        title: "Brazil Livery".into(),
        description: "A livery".into(),
        long_description: None,
        category: "liveries".into(),
        game: "MotoGP 26".into(),
        version: version.into(),
        file_name: "brazil.zip".into(),
        size,
        sha256: None,
        ownership_confirmed: true,
    }
}

const PNG: [u8; 10] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0];

/// Uploads a complete version and returns it (still pending).
async fn publish(f: &Fixture, who: &User, version: &str) -> ModVersion {
    let session = f.service.create_upload(who, raw(version, 6)).await.unwrap();
    f.service
        .write_chunk(who, &session.upload_id, 0, b"abcdef")
        .await
        .unwrap();
    f.service.complete(who, &session.upload_id).await.unwrap()
}

#[tokio::test]
async fn a_full_upload_becomes_a_pending_version() {
    let f = fixture();
    let who = creator();
    let session = f.service.create_upload(&who, raw("1.0", 6)).await.unwrap();
    assert_eq!(session.chunk_size, CHUNK_SIZE);
    assert_eq!(session.received, 0);

    assert_eq!(
        f.service
            .write_chunk(&who, &session.upload_id, 0, b"abc")
            .await
            .unwrap(),
        3
    );
    let status = f
        .service
        .upload_status(&who, &session.upload_id)
        .await
        .unwrap();
    assert_eq!((status.received, status.state), (3, UploadState::Uploading));
    f.service
        .write_chunk(&who, &session.upload_id, 3, b"def")
        .await
        .unwrap();
    let status = f
        .service
        .upload_status(&who, &session.upload_id)
        .await
        .unwrap();
    assert_eq!(status.state.as_str(), "ready");

    let version = f.service.complete(&who, &session.upload_id).await.unwrap();
    assert_eq!(version.status, VersionStatus::Pending);
    assert_eq!(version.sha256, "sum:6");
    assert!(f
        .storage
        .pending
        .lock()
        .unwrap()
        .contains_key("brazil-livery/1.0/brazil.zip"));
    assert!(f.storage.public.lock().unwrap().is_empty());
    assert!(f.storage.incoming.lock().unwrap().is_empty());

    let mine = f.service.my_mods(&who).await.unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].versions[0].status, VersionStatus::Pending);
    assert!(f.service.public_catalog().await.unwrap().is_empty());
}

#[tokio::test]
async fn invalid_metadata_maps_to_the_right_errors() {
    let f = fixture();
    let mut bad = raw("1.0", 6);
    bad.title = " ".into();
    assert!(matches!(
        f.service.create_upload(&creator(), bad).await,
        Err(AppError::BadRequest(_))
    ));
    let mut exe = raw("1.0", 6);
    exe.file_name = "a.exe".into();
    assert!(matches!(
        f.service.create_upload(&creator(), exe).await,
        Err(AppError::UnsupportedFile(_))
    ));
}

#[tokio::test]
async fn a_file_over_the_cap_is_refused_before_anything_is_stored() {
    let f = fixture();
    let result = f.service.create_upload(&creator(), raw("1.0", 1_001)).await;
    assert_eq!(result.unwrap_err(), AppError::FileTooLarge);
    let state = f.repo.0.lock().unwrap();
    assert!(state.uploads.is_empty());
    assert!(state.starts.is_empty());
    assert!(f.storage.incoming.lock().unwrap().is_empty());
}

#[tokio::test]
async fn an_interrupted_upload_resumes_from_the_received_bytes() {
    let f = fixture();
    let who = creator();
    let session = f.service.create_upload(&who, raw("1.0", 6)).await.unwrap();
    f.service
        .write_chunk(&who, &session.upload_id, 0, b"abc")
        .await
        .unwrap();
    // The client lost the answer and resends from the wrong offset.
    let error = f
        .service
        .write_chunk(&who, &session.upload_id, 0, b"abc")
        .await
        .unwrap_err();
    assert_eq!(error, AppError::OffsetMismatch { received: 3 });
    let status = f
        .service
        .upload_status(&who, &session.upload_id)
        .await
        .unwrap();
    assert_eq!(status.received, 3);
    f.service
        .write_chunk(&who, &session.upload_id, status.received, b"def")
        .await
        .unwrap();
    assert!(f.service.complete(&who, &session.upload_id).await.is_ok());
}

#[tokio::test]
async fn chunks_are_checked_for_size() {
    let f = fixture();
    let who = creator();
    let id = f
        .service
        .create_upload(&who, raw("1.0", 6))
        .await
        .unwrap()
        .upload_id;
    assert!(matches!(
        f.service.write_chunk(&who, &id, 0, b"").await,
        Err(AppError::BadRequest(_))
    ));
    assert!(matches!(
        f.service.write_chunk(&who, &id, 0, b"1234567").await,
        Err(AppError::BadRequest(_))
    ));
    let huge = vec![0_u8; CHUNK_SIZE as usize + 1];
    assert_eq!(
        f.service
            .write_chunk(&who, &id, 0, &huge)
            .await
            .unwrap_err(),
        AppError::FileTooLarge
    );
}

#[tokio::test]
async fn bytes_past_the_cap_are_refused_while_receiving() {
    // A cap lowered after the upload started still stops the bytes.
    let mut f = fixture();
    let who = creator();
    let id = f
        .service
        .create_upload(&who, raw("1.0", 900))
        .await
        .unwrap()
        .upload_id;
    f.service.limits.max_upload_bytes = 100;
    let error = f
        .service
        .write_chunk(&who, &id, 0, &[1_u8; 200])
        .await
        .unwrap_err();
    assert_eq!(error, AppError::FileTooLarge);
}

#[tokio::test]
async fn complete_needs_every_byte() {
    let f = fixture();
    let who = creator();
    let id = f
        .service
        .create_upload(&who, raw("1.0", 6))
        .await
        .unwrap()
        .upload_id;
    f.service.write_chunk(&who, &id, 0, b"abc").await.unwrap();
    assert_eq!(
        f.service.complete(&who, &id).await.unwrap_err(),
        AppError::UploadIncomplete
    );
}

#[tokio::test]
async fn complete_resyncs_when_the_disk_has_fewer_bytes_than_the_books() {
    let f = fixture();
    let who = creator();
    let id = f
        .service
        .create_upload(&who, raw("1.0", 6))
        .await
        .unwrap()
        .upload_id;
    f.service
        .write_chunk(&who, &id, 0, b"abcdef")
        .await
        .unwrap();
    f.storage
        .incoming
        .lock()
        .unwrap()
        .get_mut(&id)
        .unwrap()
        .truncate(2);
    assert_eq!(
        f.service.complete(&who, &id).await.unwrap_err(),
        AppError::UploadIncomplete
    );
    assert_eq!(
        f.service.upload_status(&who, &id).await.unwrap().received,
        2
    );
}

#[tokio::test]
async fn a_wrong_client_checksum_discards_the_upload() {
    let f = fixture();
    let who = creator();
    let mut input = raw("1.0", 6);
    input.sha256 = Some("a".repeat(64));
    let id = f
        .service
        .create_upload(&who, input)
        .await
        .unwrap()
        .upload_id;
    f.service
        .write_chunk(&who, &id, 0, b"abcdef")
        .await
        .unwrap();
    assert_eq!(
        f.service.complete(&who, &id).await.unwrap_err(),
        AppError::ChecksumMismatch
    );
    assert!(f.storage.incoming.lock().unwrap().is_empty());
    assert!(f.repo.0.lock().unwrap().uploads.is_empty());
}

#[tokio::test]
async fn only_the_owner_can_touch_an_upload() {
    let f = fixture();
    let id = f
        .service
        .create_upload(&creator(), raw("1.0", 6))
        .await
        .unwrap()
        .upload_id;
    let other = user("other", "222");
    assert_eq!(
        f.service
            .write_chunk(&other, &id, 0, b"a")
            .await
            .unwrap_err(),
        AppError::Forbidden
    );
    assert_eq!(
        f.service.upload_status(&other, &id).await.unwrap_err(),
        AppError::Forbidden
    );
    assert_eq!(
        f.service.abort(&other, "unknown").await.unwrap_err(),
        AppError::NotFound
    );
}

#[tokio::test]
async fn abort_deletes_the_bytes_and_the_session() {
    let f = fixture();
    let who = creator();
    let id = f
        .service
        .create_upload(&who, raw("1.0", 6))
        .await
        .unwrap()
        .upload_id;
    f.service.write_chunk(&who, &id, 0, b"abc").await.unwrap();
    f.service.add_screenshot(&who, &id, &PNG).await.unwrap();
    f.service.abort(&who, &id).await.unwrap();
    assert!(f.storage.incoming.lock().unwrap().is_empty());
    assert!(f.storage.pending.lock().unwrap().is_empty());
    assert_eq!(
        f.service.upload_status(&who, &id).await.unwrap_err(),
        AppError::NotFound
    );
}

#[tokio::test]
async fn expired_sessions_are_unreachable_and_purged_on_the_next_create() {
    let f = fixture();
    let who = creator();
    let id = f
        .service
        .create_upload(&who, raw("1.0", 6))
        .await
        .unwrap()
        .upload_id;
    f.service.write_chunk(&who, &id, 0, b"abc").await.unwrap();
    f.clock.advance(Duration::hours(25));
    assert_eq!(
        f.service.upload_status(&who, &id).await.unwrap_err(),
        AppError::NotFound
    );
    f.service.create_upload(&who, raw("1.1", 6)).await.unwrap();
    assert!(!f.storage.incoming.lock().unwrap().contains_key(&id));
    assert_eq!(f.repo.0.lock().unwrap().uploads.len(), 1);
}

#[tokio::test]
async fn screenshots_are_checked_by_magic_bytes_size_and_count() {
    let f = fixture();
    let who = creator();
    let id = f
        .service
        .create_upload(&who, raw("1.0", 6))
        .await
        .unwrap()
        .upload_id;
    assert!(matches!(
        f.service
            .add_screenshot(&who, &id, b"GIF89a-not-allowed")
            .await,
        Err(AppError::UnsupportedFile(_))
    ));
    let big = vec![0_u8; MAX_SCREENSHOT_BYTES as usize + 1];
    assert_eq!(
        f.service.add_screenshot(&who, &id, &big).await.unwrap_err(),
        AppError::FileTooLarge
    );
    for expected in 0..MAX_SCREENSHOTS {
        assert_eq!(
            f.service.add_screenshot(&who, &id, &PNG).await.unwrap(),
            expected
        );
    }
    assert!(matches!(
        f.service.add_screenshot(&who, &id, &PNG).await,
        Err(AppError::BadRequest(_))
    ));
    assert!(f
        .storage
        .pending
        .lock()
        .unwrap()
        .contains_key("brazil-livery/1.0/screenshot-0.png"));
}

#[tokio::test]
async fn screenshots_travel_with_the_version() {
    let f = fixture();
    let who = creator();
    let id = f
        .service
        .create_upload(&who, raw("1.0", 6))
        .await
        .unwrap()
        .upload_id;
    f.service.add_screenshot(&who, &id, &PNG).await.unwrap();
    f.service
        .write_chunk(&who, &id, 0, b"abcdef")
        .await
        .unwrap();
    let version = f.service.complete(&who, &id).await.unwrap();
    assert_eq!(version.screenshots.len(), 1);

    f.service
        .approve(&admin(), "brazil-livery", "1.0")
        .await
        .unwrap();
    let public = f.storage.public.lock().unwrap();
    assert!(public.contains_key("brazil-livery/1.0/brazil.zip"));
    assert!(public.contains_key("brazil-livery/1.0/screenshot-0.png"));
}

#[tokio::test]
async fn mod_ids_belong_to_their_creator() {
    let f = fixture();
    publish(&f, &creator(), "1.0").await;
    let other = user("other", "222");
    assert_eq!(
        f.service
            .create_upload(&other, raw("2.0", 6))
            .await
            .unwrap_err(),
        AppError::ModIdTaken
    );
}

#[tokio::test]
async fn an_existing_version_cannot_be_uploaded_again_unless_rejected() {
    let f = fixture();
    let who = creator();
    publish(&f, &who, "1.0").await;
    assert_eq!(
        f.service
            .create_upload(&who, raw("1.0", 6))
            .await
            .unwrap_err(),
        AppError::VersionExists
    );
    f.service
        .reject(&admin(), "brazil-livery", "1.0", "Wrong game")
        .await
        .unwrap();
    // A rejected version may be submitted again.
    publish(&f, &who, "1.0").await;
}

#[tokio::test]
async fn complete_rechecks_mod_ownership_and_version_conflicts() {
    let f = fixture();
    let who = creator();
    let id = f
        .service
        .create_upload(&who, raw("1.0", 6))
        .await
        .unwrap()
        .upload_id;
    f.service
        .write_chunk(&who, &id, 0, b"abcdef")
        .await
        .unwrap();
    // Meanwhile someone else took the mod id.
    f.repo
        .0
        .lock()
        .unwrap()
        .owners
        .insert("brazil-livery".into(), UserId("other".into()));
    assert_eq!(
        f.service.complete(&who, &id).await.unwrap_err(),
        AppError::ModIdTaken
    );

    let f = fixture();
    let id = f
        .service
        .create_upload(&who, raw("1.0", 6))
        .await
        .unwrap()
        .upload_id;
    f.service
        .write_chunk(&who, &id, 0, b"abcdef")
        .await
        .unwrap();
    let other_upload = f
        .service
        .create_upload(&who, raw("1.0", 6))
        .await
        .unwrap()
        .upload_id;
    f.service
        .write_chunk(&who, &other_upload, 0, b"abcdef")
        .await
        .unwrap();
    f.service.complete(&who, &other_upload).await.unwrap();
    assert_eq!(
        f.service.complete(&who, &id).await.unwrap_err(),
        AppError::VersionExists
    );
}

#[tokio::test]
async fn uploads_per_day_are_limited_with_a_retry_after() {
    let f = fixture();
    let who = creator();
    for version in ["1.0", "1.1", "1.2"] {
        f.service
            .create_upload(&who, raw(version, 6))
            .await
            .unwrap();
        f.clock.advance(Duration::hours(1));
    }
    let error = f
        .service
        .create_upload(&who, raw("1.3", 6))
        .await
        .unwrap_err();
    // The oldest start was 3 hours ago, so 21 hours remain.
    assert_eq!(
        error,
        AppError::RateLimited {
            retry_after_secs: 21 * 3600
        }
    );
    // Another user is unaffected.
    let other = user("other", "222");
    let mut input = raw("1.0", 6);
    input.mod_id = "other-mod".into();
    assert!(f.service.create_upload(&other, input).await.is_ok());
}

#[tokio::test]
async fn the_per_user_quota_counts_versions_and_open_uploads() {
    let f = fixture_with(ModLimits {
        max_upload_bytes: 1_000,
        user_quota_bytes: 1_500,
        uploads_per_day: 10,
    });
    let who = creator();
    f.service
        .create_upload(&who, raw("1.0", 1_000))
        .await
        .unwrap();
    assert_eq!(
        f.service
            .create_upload(&who, raw("1.1", 600))
            .await
            .unwrap_err(),
        AppError::QuotaExceeded
    );
    assert!(f.service.create_upload(&who, raw("1.1", 500)).await.is_ok());
}

#[tokio::test]
async fn only_admins_review() {
    let f = fixture();
    publish(&f, &creator(), "1.0").await;
    let outsider = user("other", "222");
    assert_eq!(
        f.service.list_pending(&outsider).await.unwrap_err(),
        AppError::Forbidden
    );
    assert_eq!(
        f.service
            .approve(&outsider, "brazil-livery", "1.0")
            .await
            .unwrap_err(),
        AppError::Forbidden
    );
    assert_eq!(
        f.service
            .reject(&outsider, "brazil-livery", "1.0", "no")
            .await
            .unwrap_err(),
        AppError::Forbidden
    );
    assert_eq!(f.service.list_pending(&admin()).await.unwrap().len(), 1);
}

#[tokio::test]
async fn approve_publishes_and_adds_the_mod_to_the_catalog() {
    let f = fixture();
    publish(&f, &creator(), "1.0").await;
    f.service
        .approve(&admin(), "brazil-livery", "1.0")
        .await
        .unwrap();
    assert!(f
        .storage
        .public
        .lock()
        .unwrap()
        .contains_key("brazil-livery/1.0/brazil.zip"));
    assert!(f.storage.pending.lock().unwrap().is_empty());
    let catalog = f.service.public_catalog().await.unwrap();
    assert_eq!(catalog.len(), 1);
    assert!(f.service.list_pending(&admin()).await.unwrap().is_empty());
}

#[tokio::test]
async fn reject_deletes_the_files_and_keeps_the_row_with_the_reason() {
    let f = fixture();
    let who = creator();
    publish(&f, &who, "1.0").await;
    f.service
        .reject(&admin(), "brazil-livery", "1.0", "  Not allowed  ")
        .await
        .unwrap();
    assert!(f.storage.pending.lock().unwrap().is_empty());
    let mine = f.service.my_mods(&who).await.unwrap();
    let version = &mine[0].versions[0];
    assert_eq!(version.status, VersionStatus::Rejected);
    assert_eq!(version.rejection_reason.as_deref(), Some("Not allowed"));
}

#[tokio::test]
async fn review_validates_its_input() {
    let f = fixture();
    publish(&f, &creator(), "1.0").await;
    for reason in ["", "   ", &"r".repeat(501)] {
        assert!(matches!(
            f.service
                .reject(&admin(), "brazil-livery", "1.0", reason)
                .await,
            Err(AppError::BadRequest(_))
        ));
    }
    assert_eq!(
        f.service
            .approve(&admin(), "Bad Id", "1.0")
            .await
            .unwrap_err(),
        AppError::NotFound
    );
    assert_eq!(
        f.service
            .approve(&admin(), "brazil-livery", "9.9")
            .await
            .unwrap_err(),
        AppError::NotFound
    );
    f.service
        .approve(&admin(), "brazil-livery", "1.0")
        .await
        .unwrap();
    assert!(matches!(
        f.service.approve(&admin(), "brazil-livery", "1.0").await,
        Err(AppError::BadRequest(_))
    ));
}

#[tokio::test]
async fn a_new_version_keeps_the_history() {
    let f = fixture();
    let who = creator();
    publish(&f, &who, "1.0").await;
    f.service
        .approve(&admin(), "brazil-livery", "1.0")
        .await
        .unwrap();
    publish(&f, &who, "1.1").await;
    f.service
        .approve(&admin(), "brazil-livery", "1.1")
        .await
        .unwrap();
    let mine = f.service.my_mods(&who).await.unwrap();
    assert_eq!(mine.len(), 1);
    let versions: Vec<_> = mine[0]
        .versions
        .iter()
        .map(|v| v.version.as_str())
        .collect();
    assert_eq!(versions, ["1.0", "1.1"]);
}

#[tokio::test]
async fn every_user_sees_only_their_mods() {
    let f = fixture();
    publish(&f, &creator(), "1.0").await;
    assert!(f
        .service
        .my_mods(&user("other", "222"))
        .await
        .unwrap()
        .is_empty());
}
