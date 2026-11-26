//! HTTP-level tests of moderation: reports, hidden comments, bans and the audit
//! of mod reviews (router + `oneshot`, in-memory SQLite, fake clock).

mod mod_common;

use axum::http::Method;
use chrono::Duration;
use community_application::moderation::ModerationRepository;
use community_domain::moderation::ModerationAction;
use mod_common::{limits, Env, PNG};
use serde_json::json;

async fn env() -> Env {
    Env::new(limits(1_000_000, 10_000_000, 20)).await
}

async fn count_sql(env: &Env, sql: &str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(env.store.pool())
        .await
        .unwrap()
}

#[tokio::test]
async fn reporting_answers_204_and_is_idempotent() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let bob = env.user("bob", "102").await;
    let id = env.comment_id(&ada, "livery-pack", "Spam here").await;

    let first = env.report(&bob, "livery-pack", &id, "spam").await;
    assert_eq!(first.status, 204);
    assert!(first.bytes.is_empty());
    let second = env.report(&bob, "livery-pack", &id, "abuse").await;
    assert_eq!(second.status, 204);

    assert_eq!(
        count_sql(&env, "SELECT COUNT(*) FROM comment_reports").await,
        1
    );
    let reason: String = sqlx::query_scalar("SELECT reason FROM comment_reports")
        .fetch_one(env.store.pool())
        .await
        .unwrap();
    assert_eq!(reason, "spam");
}

#[tokio::test]
async fn a_report_can_carry_details() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let bob = env.user("bob", "102").await;
    let id = env.comment_id(&ada, "livery-pack", "Rude words").await;
    let reply = env
        .post_json(
            &format!("/mods/livery-pack/comments/{id}/reports"),
            Some(&bob),
            json!({ "reason": "abuse", "details": "  insults me  " }),
        )
        .await;
    assert_eq!(reply.status, 204);
    let details: Option<String> = sqlx::query_scalar("SELECT details FROM comment_reports")
        .fetch_one(env.store.pool())
        .await
        .unwrap();
    assert_eq!(details.as_deref(), Some("insults me"));
}

#[tokio::test]
async fn bad_reports_are_400_and_missing_targets_404() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let bob = env.user("bob", "102").await;
    let id = env.comment_id(&ada, "livery-pack", "Hello").await;

    // Your own comment.
    assert_eq!(
        env.report(&ada, "livery-pack", &id, "spam").await.status,
        400
    );
    // A bad reason, bad details and a bad JSON body.
    assert_eq!(
        env.report(&bob, "livery-pack", &id, "rude").await.status,
        400
    );
    let long = env
        .post_json(
            &format!("/mods/livery-pack/comments/{id}/reports"),
            Some(&bob),
            json!({ "reason": "other", "details": "x".repeat(501) }),
        )
        .await;
    assert_eq!(long.status, 400);
    let garbage = env
        .send(
            Method::POST,
            &format!("/mods/livery-pack/comments/{id}/reports"),
            Some(&bob),
            &[],
            b"nope".to_vec(),
            true,
        )
        .await;
    assert_eq!(garbage.status, 400);
    // Unknown comment and a comment of another mod.
    assert_eq!(
        env.report(&bob, "livery-pack", "missing", "spam")
            .await
            .status,
        404
    );
    assert_eq!(env.report(&bob, "other-mod", &id, "spam").await.status, 404);
    assert_eq!(
        count_sql(&env, "SELECT COUNT(*) FROM comment_reports").await,
        0
    );
}

#[tokio::test]
async fn reporting_needs_a_session_and_a_small_body() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let bob = env.user("bob", "102").await;
    let id = env.comment_id(&ada, "livery-pack", "Hello").await;
    let uri = format!("/mods/livery-pack/comments/{id}/reports");

    let anonymous = env.post_json(&uri, None, json!({ "reason": "spam" })).await;
    assert_eq!(anonymous.status, 401);

    let huge = env
        .post_json(
            &uri,
            Some(&bob),
            json!({ "reason": "other", "details": "x".repeat(20_000) }),
        )
        .await;
    // Like comments: an over-limit body is a bad JSON body (400).
    assert_eq!(huge.status, 400);
    assert_eq!(
        count_sql(&env, "SELECT COUNT(*) FROM comment_reports").await,
        0
    );
}

#[tokio::test]
async fn reports_share_the_comment_write_limit() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let bob = env.user("bob", "102").await;
    let id = env.comment_id(&ada, "livery-pack", "Hello").await;
    for _ in 0..5 {
        assert_eq!(
            env.report(&bob, "livery-pack", &id, "spam").await.status,
            204
        );
    }
    let limited = env.report(&bob, "livery-pack", &id, "spam").await;
    assert_eq!(limited.status, 429);
    assert!(limited.headers.contains_key("retry-after"));
}

#[tokio::test]
async fn a_hidden_comment_disappears_and_is_404_for_everything() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let bob = env.user("bob", "102").await;
    env.admin().await;
    let admin = env.user_row("admin").await;
    let id = env.comment_id(&ada, "livery-pack", "Buy followers").await;
    // Same-millisecond comments are ordered by random id, so keep the order stable.
    env.advance(Duration::seconds(1));
    env.comment_id(&ada, "livery-pack", "A fine comment").await;
    env.report(&bob, "livery-pack", &id, "spam").await;

    env.moderation
        .hide_comment(&admin, &id, "advertising")
        .await
        .unwrap();

    assert_eq!(
        env.visible_comments("livery-pack").await,
        ["A fine comment"]
    );
    let uri = format!("/mods/livery-pack/comments/{id}");
    let edit = env
        .send(
            Method::PATCH,
            &uri,
            Some(&ada),
            &[],
            json!({ "body": "edited" }).to_string().into_bytes(),
            true,
        )
        .await;
    assert_eq!(edit.status, 404);
    let delete = env
        .send(Method::DELETE, &uri, Some(&ada), &[], Vec::new(), false)
        .await;
    assert_eq!(delete.status, 404);
    assert_eq!(
        env.report(&bob, "livery-pack", &id, "spam").await.status,
        404
    );

    // The open report was resolved as hidden and the audit has the details.
    let resolution: String = sqlx::query_scalar("SELECT resolution FROM comment_reports")
        .fetch_one(env.store.pool())
        .await
        .unwrap();
    assert_eq!(resolution, "hidden");
    let audit = env.store.recent_audit(10).await.unwrap();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].action, ModerationAction::HideComment);
    assert_eq!(audit[0].target, id);
    assert_eq!(audit[0].reason.as_deref(), Some("advertising"));
    assert_eq!(audit[0].admin_id, Some(admin.id.clone()));
    assert_eq!(audit[0].admin_name.as_deref(), Some("admin"));

    // Restoring shows it again, in its original place.
    env.moderation.restore_comment(&admin, &id).await.unwrap();
    assert_eq!(
        env.visible_comments("livery-pack").await,
        ["Buy followers", "A fine comment"]
    );
}

#[tokio::test]
async fn a_banned_user_is_refused_on_every_write_and_their_comments_vanish() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let bob = env.user("bob", "102").await;
    env.admin().await;
    let admin = env.user_row("admin").await;
    let ada_comment = env.comment_id(&ada, "livery-pack", "Ada was here").await;
    env.advance(Duration::seconds(1));
    env.comment_id(&bob, "livery-pack", "Bob was here").await;
    env.advance(Duration::seconds(1));
    let bob_comment = env.comment_id(&bob, "livery-pack", "Bob again").await;
    // An upload started before the ban.
    let started = env.start(&ada, "brazil-livery", "1.0", 11).await;
    let upload = started.json()["uploadId"].as_str().unwrap().to_owned();

    let ada_row = env.user_row("ada").await;
    env.moderation
        .ban_user(&admin, ada_row.id.as_str(), "spam wave")
        .await
        .unwrap();

    let refused = |reply: mod_common::Reply| {
        assert_eq!(reply.status, 403, "{:?}", reply.json());
        assert_eq!(reply.error_code(), "banned");
    };
    refused(env.comment(&ada, "livery-pack", "Still here?").await);
    refused(
        env.send(
            Method::PATCH,
            &format!("/mods/livery-pack/comments/{ada_comment}"),
            Some(&ada),
            &[],
            json!({ "body": "edit" }).to_string().into_bytes(),
            true,
        )
        .await,
    );
    refused(
        env.send(
            Method::DELETE,
            &format!("/mods/livery-pack/comments/{ada_comment}"),
            Some(&ada),
            &[],
            Vec::new(),
            false,
        )
        .await,
    );
    refused(env.report(&ada, "livery-pack", &bob_comment, "spam").await);
    for method in [Method::PUT, Method::DELETE] {
        refused(
            env.send(
                method,
                "/mods/livery-pack/star",
                Some(&ada),
                &[],
                Vec::new(),
                false,
            )
            .await,
        );
    }
    refused(env.start(&ada, "brazil-livery", "2.0", 11).await);
    refused(env.put_chunk(&ada, &upload, 0, b"hello world").await);
    refused(
        env.send(
            Method::POST,
            &format!("/mods/uploads/{upload}/screenshots"),
            Some(&ada),
            &[],
            PNG.to_vec(),
            false,
        )
        .await,
    );
    refused(
        env.post_json(
            &format!("/mods/uploads/{upload}/complete"),
            Some(&ada),
            json!({}),
        )
        .await,
    );
    refused(
        env.send(
            Method::DELETE,
            &format!("/mods/uploads/{upload}"),
            Some(&ada),
            &[],
            Vec::new(),
            false,
        )
        .await,
    );

    // Reads keep working for the banned user, other users are unaffected.
    assert_eq!(env.get("/me", Some(&ada)).await.status, 200);
    assert_eq!(env.get("/mods/uploads/x", Some(&ada)).await.status, 404);
    assert_eq!(
        env.visible_comments("livery-pack").await,
        ["Bob was here", "Bob again"]
    );
    env.comment_id(&bob, "livery-pack", "Bob still posts").await;

    // Unbanning brings the writes and the comments back.
    env.moderation
        .unban(&admin, ada_row.id.as_str())
        .await
        .unwrap();
    assert_eq!(env.visible_comments("livery-pack").await[0], "Ada was here");
    env.advance(Duration::minutes(1));
    assert_eq!(
        env.comment(&ada, "livery-pack", "I am back").await.status,
        201
    );
    assert_eq!(
        env.send(
            Method::PUT,
            "/mods/livery-pack/star",
            Some(&ada),
            &[],
            Vec::new(),
            false
        )
        .await
        .status,
        200
    );
}

#[tokio::test]
async fn the_ban_check_comes_before_the_write_limit() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    env.admin().await;
    let admin = env.user_row("admin").await;
    let ada_row = env.user_row("ada").await;
    env.moderation
        .ban_user(&admin, ada_row.id.as_str(), "x")
        .await
        .unwrap();
    // More attempts than the 5 writes per minute: still `banned`, never 429.
    for _ in 0..8 {
        let reply = env.comment(&ada, "livery-pack", "hi").await;
        assert_eq!(reply.status, 403);
        assert_eq!(reply.error_code(), "banned");
    }
}

#[tokio::test]
async fn banned_users_comments_do_not_break_the_cursor() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let bob = env.user("bob", "102").await;
    env.admin().await;
    let admin = env.user_row("admin").await;
    for (token, text) in [
        (&bob, "b1"),
        (&ada, "a1"),
        (&bob, "b2"),
        (&ada, "a2"),
        (&bob, "b3"),
    ] {
        env.comment_id(token, "livery-pack", text).await;
        env.advance(Duration::seconds(1));
    }
    let ada_row = env.user_row("ada").await;
    env.moderation
        .ban_user(&admin, ada_row.id.as_str(), "x")
        .await
        .unwrap();

    let mut seen = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let uri = match &cursor {
            Some(c) => format!("/mods/livery-pack/comments?limit=1&cursor={c}"),
            None => "/mods/livery-pack/comments?limit=1".to_owned(),
        };
        let page = env.get(&uri, None).await.json();
        seen.extend(
            page["comments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c["body"].as_str().unwrap().to_owned()),
        );
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, ["b1", "b2", "b3"]);
}

#[tokio::test]
async fn approving_and_rejecting_a_version_are_audited() {
    let env = env().await;
    let creator = env.user("creator", "111").await;
    let admin = env.admin().await;
    env.publish(&creator, "brazil-livery", "1.0", b"hello world", 11)
        .await;
    env.publish(&creator, "brazil-livery", "2.0", b"hello again", 11)
        .await;

    assert_eq!(
        env.approve(&admin, "brazil-livery", "1.0").await.status,
        200
    );
    assert_eq!(
        env.reject(&admin, "brazil-livery", "2.0", "  Ripped asset ")
            .await
            .status,
        200
    );

    let audit = env.store.recent_audit(10).await.unwrap();
    assert_eq!(audit.len(), 2);
    assert_eq!(audit[0].action, ModerationAction::RejectVersion);
    assert_eq!(audit[0].target, "brazil-livery@2.0");
    assert_eq!(audit[0].reason.as_deref(), Some("Ripped asset"));
    assert_eq!(audit[1].action, ModerationAction::ApproveVersion);
    assert_eq!(audit[1].target, "brazil-livery@1.0");
    assert_eq!(audit[1].admin_name.as_deref(), Some("admin"));

    // A refused review (already reviewed) is not audited.
    assert_eq!(
        env.approve(&admin, "brazil-livery", "1.0").await.status,
        400
    );
    assert_eq!(env.store.recent_audit(10).await.unwrap().len(), 2);
}
