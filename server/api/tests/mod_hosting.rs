//! HTTP-level tests of mod hosting, driven through the router with `oneshot`.

mod mod_common;

use axum::http::Method;
use chrono::Duration;
use mod_common::{limits, Env, PNG};
use serde_json::json;

const HELLO_SHA256: &str = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";

async fn env() -> Env {
    Env::new(limits(1_000_000, 10_000_000, 20)).await
}

#[tokio::test]
async fn every_upload_and_review_route_needs_a_session() {
    let env = env().await;
    for (method, uri) in [
        (Method::POST, "/mods/uploads"),
        (Method::PUT, "/mods/uploads/x/chunks?offset=0"),
        (Method::GET, "/mods/uploads/x"),
        (Method::DELETE, "/mods/uploads/x"),
        (Method::POST, "/mods/uploads/x/screenshots"),
        (Method::POST, "/mods/uploads/x/complete"),
        (Method::GET, "/mods/mine"),
        (Method::GET, "/admin/mods/pending"),
        (Method::POST, "/admin/mods/m/versions/1/approve"),
        (Method::POST, "/admin/mods/m/versions/1/reject"),
    ] {
        let reply = env.send(method, uri, None, &[], Vec::new(), false).await;
        assert_eq!(reply.status, 401, "{uri}");
        assert_eq!(reply.error_code(), "unauthorized");
    }
}

#[tokio::test]
async fn creating_an_upload_answers_201_with_the_chunk_size() {
    let env = env().await;
    let token = env.user("creator", "111").await;
    let reply = env.start(&token, "brazil-livery", "1.0", 11).await;
    assert_eq!(reply.status, 201);
    let json = reply.json();
    assert_eq!(json["chunkSize"], 8 * 1024 * 1024);
    assert_eq!(json["received"], 0);
    assert!(json["uploadId"].as_str().unwrap().len() >= 16);
    assert!(json["expiresAt"].as_str().unwrap().ends_with('Z'));

    let bad = env
        .post_json("/mods/uploads", Some(&token), json!({"x": 1}))
        .await;
    assert_eq!(
        (bad.status, bad.error_code().as_str()),
        (400, "bad_request")
    );
    let mut missing_confirmation = Env::create_body("a-mod", "1.0", 5);
    missing_confirmation["ownershipConfirmed"] = json!(false);
    let reply = env
        .post_json("/mods/uploads", Some(&token), missing_confirmation)
        .await;
    assert_eq!(reply.status, 400);
}

#[tokio::test]
async fn a_zip_goes_through_upload_complete_and_shows_in_my_mods_as_pending() {
    let env = env().await;
    let token = env.user("creator", "111").await;
    let version = env
        .publish(&token, "brazil-livery", "1.0", b"hello world", 4)
        .await;
    assert_eq!(version["status"], "pending");
    assert_eq!(version["sha256"], HELLO_SHA256);
    assert_eq!(version["size"], 11);

    let mine = env.get("/mods/mine", Some(&token)).await.json();
    assert_eq!(mine["mods"][0]["id"], "brazil-livery");
    assert_eq!(mine["mods"][0]["versions"][0]["status"], "pending");
    let stored = env
        .dir
        .path()
        .join("pending/brazil-livery/1.0/brazil-livery.zip");
    assert_eq!(std::fs::read(stored).unwrap(), b"hello world");
}

#[tokio::test]
async fn a_matching_client_checksum_is_accepted_and_a_wrong_one_is_refused() {
    let env = env().await;
    let token = env.user("creator", "111").await;
    let mut body = Env::create_body("brazil-livery", "1.0", 11);
    body["sha256"] = json!(HELLO_SHA256.to_uppercase());
    let id = env
        .post_json("/mods/uploads", Some(&token), body)
        .await
        .json()["uploadId"]
        .as_str()
        .unwrap()
        .to_owned();
    env.put_chunk(&token, &id, 0, b"hello world").await;
    let done = env
        .post_json(
            &format!("/mods/uploads/{id}/complete"),
            Some(&token),
            json!({}),
        )
        .await;
    assert_eq!(done.status, 201);

    let mut wrong = Env::create_body("other-mod", "1.0", 11);
    wrong["sha256"] = json!("0".repeat(64));
    let id = env
        .post_json("/mods/uploads", Some(&token), wrong)
        .await
        .json()["uploadId"]
        .as_str()
        .unwrap()
        .to_owned();
    env.put_chunk(&token, &id, 0, b"hello world").await;
    let done = env
        .post_json(
            &format!("/mods/uploads/{id}/complete"),
            Some(&token),
            json!({}),
        )
        .await;
    assert_eq!(
        (done.status, done.error_code().as_str()),
        (400, "checksum_mismatch")
    );
    assert!(!env.dir.path().join("incoming").join(&id).exists());
}

#[tokio::test]
async fn a_file_over_the_cap_is_refused_before_any_byte_is_stored() {
    let env = Env::new(limits(1_000, 10_000_000, 20)).await;
    let token = env.user("creator", "111").await;
    let reply = env.start(&token, "brazil-livery", "1.0", 1_001).await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (413, "file_too_large")
    );
    assert_eq!(
        std::fs::read_dir(env.dir.path().join("incoming"))
            .unwrap()
            .count(),
        0
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM mod_uploads")
        .fetch_one(env.store.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn only_zip_7z_and_pak_are_accepted() {
    let env = env().await;
    let token = env.user("creator", "111").await;
    for (name, expected) in [
        ("a.exe", 400),
        ("a.ZIP", 201),
        ("a.7z", 201),
        ("a.pak", 201),
    ] {
        let mut body = Env::create_body("brazil-livery", &format!("v-{name}"), 5);
        body["fileName"] = json!(name);
        let reply = env.post_json("/mods/uploads", Some(&token), body).await;
        assert_eq!(reply.status, expected, "{name}");
        if expected == 400 {
            assert_eq!(reply.error_code(), "unsupported_file");
        }
    }
}

#[tokio::test]
async fn an_interrupted_upload_resumes_from_the_bytes_the_server_has() {
    let env = env().await;
    let token = env.user("creator", "111").await;
    let id = env.start(&token, "brazil-livery", "1.0", 11).await.json()["uploadId"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        env.put_chunk(&token, &id, 0, b"hello").await.json()["received"],
        5
    );

    // The connection dropped: ask the server where to continue.
    let status = env
        .get(&format!("/mods/uploads/{id}"), Some(&token))
        .await
        .json();
    assert_eq!(status["received"], 5);
    assert_eq!(status["size"], 11);
    assert_eq!(status["state"], "uploading");

    // A stale offset is refused and tells the client the truth.
    let stale = env.put_chunk(&token, &id, 0, b"hello").await;
    assert_eq!(
        (stale.status, stale.error_code().as_str()),
        (409, "offset_mismatch")
    );
    assert_eq!(stale.json()["error"]["received"], 5);

    env.put_chunk(&token, &id, 5, b" world").await;
    let status = env
        .get(&format!("/mods/uploads/{id}"), Some(&token))
        .await
        .json();
    assert_eq!(status["state"], "ready");
    let done = env
        .post_json(
            &format!("/mods/uploads/{id}/complete"),
            Some(&token),
            json!({}),
        )
        .await;
    assert_eq!(done.json()["sha256"], HELLO_SHA256);
}

#[tokio::test]
async fn chunks_are_bounded_by_size_and_by_the_declared_file() {
    let env = Env::new(limits(20 * 1024 * 1024, 100 * 1024 * 1024, 20)).await;
    let token = env.user("creator", "111").await;
    let id = env.start(&token, "brazil-livery", "1.0", 10).await.json()["uploadId"]
        .as_str()
        .unwrap()
        .to_owned();
    let past = env.put_chunk(&token, &id, 0, b"12345678901").await;
    assert_eq!(
        (past.status, past.error_code().as_str()),
        (400, "bad_request")
    );
    let empty = env.put_chunk(&token, &id, 0, b"").await;
    assert_eq!(empty.status, 400);
    let no_offset = env
        .send(
            Method::PUT,
            &format!("/mods/uploads/{id}/chunks"),
            Some(&token),
            &[],
            b"x".to_vec(),
            false,
        )
        .await;
    assert_eq!(no_offset.status, 400);

    // The body limit of the chunk route is the chunk size (8 MiB), not axum's 2 MB default.
    let big = env
        .start(&token, "big-mod", "1.0", 9 * 1024 * 1024)
        .await
        .json()["uploadId"]
        .as_str()
        .unwrap()
        .to_owned();
    let ok = env
        .put_chunk(&token, &big, 0, &vec![7_u8; 8 * 1024 * 1024])
        .await;
    assert_eq!(ok.status, 200);
    let too_big = env
        .put_chunk(
            &token,
            &big,
            8 * 1024 * 1024,
            &vec![7_u8; 8 * 1024 * 1024 + 1],
        )
        .await;
    assert_eq!(
        (too_big.status, too_big.error_code().as_str()),
        (413, "file_too_large")
    );
}

#[tokio::test]
async fn chunk_routes_skip_the_ip_limit_but_other_routes_do_not() {
    let env = env().await;
    let token = env.user("creator", "111").await;
    let id = env.start(&token, "brazil-livery", "1.0", 100).await.json()["uploadId"]
        .as_str()
        .unwrap()
        .to_owned();
    for offset in 0..70 {
        let reply = env.put_chunk(&token, &id, offset, b"x").await;
        assert_eq!(reply.status, 200, "chunk {offset}");
    }
}

#[tokio::test]
async fn another_user_cannot_use_my_upload() {
    let env = env().await;
    let creator = env.user("creator", "111").await;
    let other = env.user("other", "222").await;
    let id = env.start(&creator, "brazil-livery", "1.0", 10).await.json()["uploadId"]
        .as_str()
        .unwrap()
        .to_owned();
    let reply = env.put_chunk(&other, &id, 0, b"x").await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (403, "forbidden")
    );
    assert_eq!(
        env.get(&format!("/mods/uploads/{id}"), Some(&other))
            .await
            .status,
        403
    );
    let unknown = env.get("/mods/uploads/unknown", Some(&other)).await;
    assert_eq!(unknown.status, 404);
}

#[tokio::test]
async fn aborting_deletes_the_incoming_bytes() {
    let env = env().await;
    let token = env.user("creator", "111").await;
    let id = env.start(&token, "brazil-livery", "1.0", 10).await.json()["uploadId"]
        .as_str()
        .unwrap()
        .to_owned();
    env.put_chunk(&token, &id, 0, b"abc").await;
    assert!(env.dir.path().join("incoming").join(&id).exists());
    let reply = env
        .send(
            Method::DELETE,
            &format!("/mods/uploads/{id}"),
            Some(&token),
            &[],
            Vec::new(),
            false,
        )
        .await;
    assert_eq!(reply.status, 204);
    assert!(!env.dir.path().join("incoming").join(&id).exists());
    assert_eq!(
        env.get(&format!("/mods/uploads/{id}"), Some(&token))
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn completing_too_early_is_a_conflict() {
    let env = env().await;
    let token = env.user("creator", "111").await;
    let id = env.start(&token, "brazil-livery", "1.0", 10).await.json()["uploadId"]
        .as_str()
        .unwrap()
        .to_owned();
    env.put_chunk(&token, &id, 0, b"abc").await;
    let reply = env
        .post_json(
            &format!("/mods/uploads/{id}/complete"),
            Some(&token),
            json!({}),
        )
        .await;
    assert_eq!(
        (reply.status, reply.error_code().as_str()),
        (409, "upload_incomplete")
    );
}

#[tokio::test]
async fn screenshots_are_validated_and_published_with_the_mod() {
    let env = env().await;
    let token = env.user("creator", "111").await;
    let id = env.start(&token, "brazil-livery", "1.0", 11).await.json()["uploadId"]
        .as_str()
        .unwrap()
        .to_owned();
    let shots = format!("/mods/uploads/{id}/screenshots");
    let not_image = env
        .send(
            Method::POST,
            &shots,
            Some(&token),
            &[],
            b"GIF89a....".to_vec(),
            false,
        )
        .await;
    assert_eq!(
        (not_image.status, not_image.error_code().as_str()),
        (400, "unsupported_file")
    );
    let ok = env
        .send(Method::POST, &shots, Some(&token), &[], PNG.to_vec(), false)
        .await;
    assert_eq!(ok.status, 201);
    assert_eq!(ok.json()["index"], 0);
    // More than 5 MiB is refused (the route's own body limit).
    let huge = env
        .send(
            Method::POST,
            &shots,
            Some(&token),
            &[],
            vec![0_u8; 5 * 1024 * 1024 + 1],
            false,
        )
        .await;
    assert_eq!(
        (huge.status, huge.error_code().as_str()),
        (413, "file_too_large")
    );
    for _ in 1..8 {
        let more = env
            .send(Method::POST, &shots, Some(&token), &[], PNG.to_vec(), false)
            .await;
        assert_eq!(more.status, 201);
    }
    let ninth = env
        .send(Method::POST, &shots, Some(&token), &[], PNG.to_vec(), false)
        .await;
    assert_eq!(ninth.status, 400);

    env.put_chunk(&token, &id, 0, b"hello world").await;
    env.post_json(
        &format!("/mods/uploads/{id}/complete"),
        Some(&token),
        json!({}),
    )
    .await;
    let admin = env.admin().await;
    assert_eq!(
        env.approve(&admin, "brazil-livery", "1.0").await.status,
        200
    );

    let catalog = env.get("/catalog", None).await.json();
    let entry = &catalog["entries"][0];
    assert_eq!(entry["screenshots"].as_array().unwrap().len(), 8);
    assert_eq!(
        entry["thumbnailUrl"],
        "https://community.test/files/brazil-livery/1.0/screenshot-0.png"
    );
    let shot = env
        .get("/files/brazil-livery/1.0/screenshot-0.png", None)
        .await;
    assert_eq!(shot.status, 200);
    assert_eq!(shot.bytes, PNG);
}

#[tokio::test]
async fn a_pending_mod_is_neither_public_nor_downloadable() {
    let env = env().await;
    let token = env.user("creator", "111").await;
    env.publish(&token, "brazil-livery", "1.0", b"hello world", 11)
        .await;

    let catalog = env.get("/catalog", None).await.json();
    assert_eq!(catalog["entries"].as_array().unwrap().len(), 0);
    for uri in [
        "/files/brazil-livery/1.0/brazil-livery.zip",
        "/files/pending/brazil-livery/1.0/brazil-livery.zip",
        "/files/../pending/brazil-livery/1.0/brazil-livery.zip",
        "/files/%2e%2e/pending/brazil-livery/1.0/brazil-livery.zip",
        "/files/..%2Fpending%2Fbrazil-livery%2F1.0%2Fbrazil-livery.zip",
        "/mods/brazil-livery",
    ] {
        let reply = env.get(uri, None).await;
        assert_eq!(reply.status, 404, "{uri}");
    }
}

#[tokio::test]
async fn only_admins_can_review() {
    let env = env().await;
    let creator = env.user("creator", "111").await;
    env.publish(&creator, "brazil-livery", "1.0", b"hello world", 11)
        .await;
    for reply in [
        env.get("/admin/mods/pending", Some(&creator)).await,
        env.approve(&creator, "brazil-livery", "1.0").await,
        env.reject(&creator, "brazil-livery", "1.0", "no").await,
    ] {
        assert_eq!(
            (reply.status, reply.error_code().as_str()),
            (403, "forbidden")
        );
    }
    let admin = env.admin().await;
    let pending = env.get("/admin/mods/pending", Some(&admin)).await.json();
    assert_eq!(pending["versions"].as_array().unwrap().len(), 1);
    assert_eq!(pending["versions"][0]["modId"], "brazil-livery");
}

#[tokio::test]
async fn approving_publishes_the_file_and_adds_it_to_the_catalog() {
    let env = env().await;
    let creator = env.user("creator", "111").await;
    let admin = env.admin().await;
    env.publish(&creator, "brazil-livery", "1.0", b"hello world", 11)
        .await;
    assert_eq!(
        env.approve(&admin, "brazil-livery", "1.0").await.status,
        200
    );

    let catalog = env.get("/catalog", None).await.json();
    assert_eq!(catalog["schemaVersion"], 1);
    let entry = &catalog["entries"][0];
    assert_eq!(entry["id"], "brazil-livery");
    assert_eq!(entry["author"], "creator");
    assert_eq!(entry["version"], "1.0");
    assert_eq!(entry["updatedAt"], "2023-11-14");
    assert_eq!(
        entry["pageUrl"],
        "https://community.test/mods/brazil-livery"
    );
    assert_eq!(entry["download"]["sha256"], HELLO_SHA256);
    assert_eq!(entry["download"]["size"], 11);
    assert_eq!(
        entry["download"]["url"],
        "https://community.test/files/brazil-livery/1.0/brazil-livery.zip"
    );

    let file = env
        .get("/files/brazil-livery/1.0/brazil-livery.zip", None)
        .await;
    assert_eq!(
        (file.status, file.bytes.as_slice()),
        (200, b"hello world".as_slice())
    );
    assert!(!env.dir.path().join("pending/brazil-livery").exists());

    // Approved twice: only a pending version can be reviewed.
    let again = env.approve(&admin, "brazil-livery", "1.0").await;
    assert_eq!(again.status, 400);
    assert_eq!(
        env.approve(&admin, "brazil-livery", "9.9").await.status,
        404
    );

    let page = env.get("/mods/brazil-livery", None).await;
    assert_eq!(page.status, 200);
    assert!(String::from_utf8(page.bytes)
        .unwrap()
        .contains("Brazil Livery"));
}

#[tokio::test]
async fn rejecting_deletes_the_file_and_tells_the_creator_why() {
    let env = env().await;
    let creator = env.user("creator", "111").await;
    let admin = env.admin().await;
    env.publish(&creator, "brazil-livery", "1.0", b"hello world", 11)
        .await;

    let blank = env.reject(&admin, "brazil-livery", "1.0", "  ").await;
    assert_eq!(blank.status, 400);
    let bad_body = env
        .send(
            Method::POST,
            "/admin/mods/brazil-livery/versions/1.0/reject",
            Some(&admin),
            &[],
            b"nope".to_vec(),
            true,
        )
        .await;
    assert_eq!(bad_body.status, 400);

    assert_eq!(
        env.reject(&admin, "brazil-livery", "1.0", "Contains a ripped asset")
            .await
            .status,
        200
    );
    assert!(!env.dir.path().join("pending/brazil-livery").exists());
    assert_eq!(
        env.get("/files/brazil-livery/1.0/brazil-livery.zip", None)
            .await
            .status,
        404
    );

    let mine = env.get("/mods/mine", Some(&creator)).await.json();
    let version = &mine["mods"][0]["versions"][0];
    assert_eq!(version["status"], "rejected");
    assert_eq!(version["rejectionReason"], "Contains a ripped asset");
    let catalog = env.get("/catalog", None).await.json();
    assert!(catalog["entries"].as_array().unwrap().is_empty());

    // The rejected number can be submitted again.
    env.publish(&creator, "brazil-livery", "1.0", b"hello world", 11)
        .await;
}

#[tokio::test]
async fn a_new_version_keeps_the_history_and_the_catalog_shows_the_latest() {
    let env = env().await;
    let creator = env.user("creator", "111").await;
    let admin = env.admin().await;
    env.publish(&creator, "brazil-livery", "1.0", b"old file", 8)
        .await;
    env.approve(&admin, "brazil-livery", "1.0").await;
    env.advance(Duration::hours(1));
    env.publish(&creator, "brazil-livery", "1.1", b"hello world", 11)
        .await;
    // Pending 1.1 does not replace the approved 1.0 yet.
    let catalog = env.get("/catalog", None).await.json();
    assert_eq!(catalog["entries"][0]["version"], "1.0");
    env.approve(&admin, "brazil-livery", "1.1").await;

    let catalog = env.get("/catalog", None).await.json();
    assert_eq!(catalog["entries"].as_array().unwrap().len(), 1);
    assert_eq!(catalog["entries"][0]["version"], "1.1");
    let mine = env.get("/mods/mine", Some(&creator)).await.json();
    let versions = mine["mods"][0]["versions"].as_array().unwrap();
    let numbers: Vec<_> = versions
        .iter()
        .map(|v| v["version"].as_str().unwrap())
        .collect();
    assert_eq!(numbers, ["1.0", "1.1"]);
    // Old files stay downloadable.
    assert_eq!(
        env.get("/files/brazil-livery/1.0/brazil-livery.zip", None)
            .await
            .status,
        200
    );
}

#[tokio::test]
async fn the_catalog_supports_etag_revalidation() {
    let env = env().await;
    let creator = env.user("creator", "111").await;
    let admin = env.admin().await;
    let first = env.get("/catalog", None).await;
    let etag = first.headers["etag"].to_str().unwrap().to_owned();
    assert!(etag.starts_with('"') && etag.ends_with('"'));
    assert_eq!(first.headers["content-type"], "application/json");

    let same = env
        .send(
            Method::GET,
            "/catalog",
            None,
            &[("if-none-match", &etag)],
            Vec::new(),
            false,
        )
        .await;
    assert_eq!(same.status, 304);
    assert!(same.bytes.is_empty());
    assert_eq!(same.headers["etag"].to_str().unwrap(), etag);

    env.publish(&creator, "brazil-livery", "1.0", b"hello world", 11)
        .await;
    env.approve(&admin, "brazil-livery", "1.0").await;
    let changed = env
        .send(
            Method::GET,
            "/catalog",
            None,
            &[("if-none-match", &etag)],
            Vec::new(),
            false,
        )
        .await;
    assert_eq!(changed.status, 200);
    assert_ne!(changed.headers["etag"].to_str().unwrap(), etag);
}

#[tokio::test]
async fn downloads_support_ranges() {
    let env = env().await;
    let creator = env.user("creator", "111").await;
    let admin = env.admin().await;
    let content: Vec<u8> = (0..250_u32).map(|n| (n % 251) as u8).collect();
    env.publish(&creator, "brazil-livery", "1.0", &content, 100)
        .await;
    env.approve(&admin, "brazil-livery", "1.0").await;
    let uri = "/files/brazil-livery/1.0/brazil-livery.zip";

    let part = env
        .send(
            Method::GET,
            uri,
            None,
            &[("range", "bytes=0-99")],
            Vec::new(),
            false,
        )
        .await;
    assert_eq!(part.status, 206);
    assert_eq!(part.bytes, content[..100]);
    assert_eq!(part.headers["content-range"], "bytes 0-99/250");

    let full = env.get(uri, None).await;
    assert_eq!(full.status, 200);
    assert_eq!(full.headers["accept-ranges"], "bytes");
    assert_eq!(full.bytes, content);

    let outside = env
        .send(
            Method::GET,
            uri,
            None,
            &[("range", "bytes=1000-2000")],
            Vec::new(),
            false,
        )
        .await;
    assert_eq!(outside.status, 416);
}

#[tokio::test]
async fn mod_ids_and_versions_are_protected() {
    let env = env().await;
    let creator = env.user("creator", "111").await;
    let other = env.user("other", "222").await;
    env.publish(&creator, "brazil-livery", "1.0", b"hello world", 11)
        .await;

    let taken = env.start(&other, "brazil-livery", "2.0", 5).await;
    assert_eq!(
        (taken.status, taken.error_code().as_str()),
        (409, "mod_id_taken")
    );
    let exists = env.start(&creator, "brazil-livery", "1.0", 5).await;
    assert_eq!(
        (exists.status, exists.error_code().as_str()),
        (409, "version_exists")
    );
    let reserved = env.start(&creator, "mine", "1.0", 5).await;
    assert_eq!(reserved.status, 400);
    let slug = env.start(&creator, "Bad Slug", "1.0", 5).await;
    assert_eq!(slug.status, 400);
    assert!(env.get("/mods/mine", Some(&other)).await.json()["mods"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn the_per_user_quota_and_the_daily_limit_are_enforced() {
    let env = Env::new(limits(1_000, 1_500, 2)).await;
    let token = env.user("creator", "111").await;
    assert_eq!(env.start(&token, "mod-a", "1.0", 1_000).await.status, 201);
    let over = env.start(&token, "mod-b", "1.0", 600).await;
    assert_eq!(
        (over.status, over.error_code().as_str()),
        (413, "quota_exceeded")
    );
    assert_eq!(env.start(&token, "mod-b", "1.0", 500).await.status, 201);

    let limited = env.start(&token, "mod-c", "1.0", 1).await;
    assert_eq!(
        (limited.status, limited.error_code().as_str()),
        (429, "rate_limited")
    );
    assert_eq!(limited.headers["retry-after"], "86400");
    env.advance(Duration::hours(25));
    assert_eq!(env.start(&token, "mod-c", "1.0", 1).await.status, 201);
}

#[tokio::test]
async fn expired_sessions_are_purged_with_their_bytes() {
    let env = env().await;
    let token = env.user("creator", "111").await;
    let id = env.start(&token, "brazil-livery", "1.0", 10).await.json()["uploadId"]
        .as_str()
        .unwrap()
        .to_owned();
    env.put_chunk(&token, &id, 0, b"abc").await;
    env.advance(Duration::hours(25));
    assert_eq!(
        env.get(&format!("/mods/uploads/{id}"), Some(&token))
            .await
            .status,
        404
    );
    env.start(&token, "other-mod", "1.0", 5).await;
    assert!(!env.dir.path().join("incoming").join(&id).exists());
}
