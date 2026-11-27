//! HTTP-level tests of the admin web (login, cookie, CSRF, dashboard, actions,
//! review files, headers). Router + `oneshot`, in-memory SQLite, fake clock and
//! fake Discord.

mod mod_common;

use community_application::moderation::ModerationRepository;
use mod_common::{cookie_from, limits, AdminBrowser, Env, Reply, ADMIN_DISCORD_ID, PNG};

async fn env() -> Env {
    Env::new(limits(1_000_000, 10_000_000, 20)).await
}

async fn count(env: &Env, sql: &str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(env.store.pool())
        .await
        .unwrap()
}

fn assert_hardened(reply: &Reply) {
    assert_eq!(reply.header("cache-control"), "no-store");
    assert_eq!(reply.header("x-frame-options"), "DENY");
    assert_eq!(reply.header("referrer-policy"), "no-referrer");
    assert_eq!(reply.header("x-content-type-options"), "nosniff");
    assert_eq!(
        reply.header("content-security-policy"),
        "default-src 'none'; img-src 'self'; style-src 'unsafe-inline'; form-action 'self'; \
         frame-ancestors 'none'; base-uri 'none'"
    );
}

fn cookie_header(token: &str) -> String {
    format!("admin_session={token}")
}

// ---- Login ----

#[tokio::test]
async fn the_login_route_redirects_to_discord_and_sets_the_login_cookie() {
    let env = env().await;
    let (reply, state) = env.admin_login_start().await;
    assert_eq!(reply.status, 302);
    let location = reply.header("location");
    assert!(location.starts_with("https://discord.test/oauth2/authorize?state="));
    assert!(location.contains(&state));
    let cookie = reply
        .set_cookies()
        .into_iter()
        .find(|c| c.starts_with("admin_login="))
        .unwrap();
    for part in [
        "HttpOnly",
        "Secure",
        "SameSite=Lax",
        "Path=/",
        "Max-Age=600",
    ] {
        assert!(cookie.contains(part), "{cookie}");
    }
    assert_hardened(&reply);
    // The flow is stored as an admin flow.
    let purpose: String = sqlx::query_scalar("SELECT purpose FROM oauth_flows")
        .fetch_one(env.store.pool())
        .await
        .unwrap();
    assert_eq!(purpose, "admin");
}

#[tokio::test]
async fn an_admin_gets_a_strict_session_cookie_and_a_forwarding_page() {
    let env = env().await;
    let reply = env.admin_sign_in(ADMIN_DISCORD_ID, "Mod").await;
    assert_eq!(reply.status, 200);
    assert!(reply
        .text()
        .contains("<meta http-equiv=\"refresh\" content=\"0;url=/admin\">"));
    assert!(reply.text().contains("href=\"/admin\""));
    let cookies = reply.set_cookies();
    let session = cookies
        .iter()
        .find(|c| c.starts_with("admin_session="))
        .unwrap();
    for part in [
        "HttpOnly",
        "Secure",
        "SameSite=Strict",
        "Path=/admin",
        "Max-Age=43200",
    ] {
        assert!(session.contains(part), "{session}");
    }
    let cleared = cookies
        .iter()
        .find(|c| c.starts_with("admin_login="))
        .unwrap();
    assert!(cleared.contains("Max-Age=0"), "{cleared}");
    assert_hardened(&reply);
    // The session lasts 12 hours.
    let (created, expires): (i64, i64) =
        sqlx::query_as("SELECT created_at, expires_at FROM sessions")
            .fetch_one(env.store.pool())
            .await
            .unwrap();
    // Stored in seconds or milliseconds depending on the column.
    assert!([43_200, 43_200_000].contains(&(expires - created)));
}

#[tokio::test]
async fn a_non_admin_who_signs_in_gets_403_and_no_session() {
    let env = env().await;
    let reply = env.admin_sign_in("555", "Rider").await;
    assert_eq!(reply.status, 403);
    assert!(reply.text().contains("not an administrator"));
    assert!(cookie_from(&reply.set_cookies(), "admin_session").is_none());
    assert_hardened(&reply);
    assert_eq!(count(&env, "SELECT COUNT(*) FROM sessions").await, 0);
}

#[tokio::test]
async fn the_callback_needs_the_matching_login_cookie() {
    let env = env().await;
    // No cookie at all (the JSON error: nothing says this is a browser sign-in).
    let (_, state) = env.admin_login_start().await;
    let missing = env.admin_callback(&state, None).await;
    assert_eq!(missing.status, 400);
    // A cookie for another state (rendered as a page).
    let (_, state) = env.admin_login_start().await;
    let wrong = env.admin_callback(&state, Some("admin_login=other")).await;
    assert_eq!(wrong.status, 400);
    assert!(wrong.text().contains("<html"));
    assert!(cookie_from(&wrong.set_cookies(), "admin_session").is_none());
    assert_eq!(count(&env, "SELECT COUNT(*) FROM sessions").await, 0);
    // The flow was consumed: even the right cookie cannot reuse the state.
    let reuse = env
        .admin_callback(&state, Some(&format!("admin_login={state}")))
        .await;
    assert_eq!(reuse.status, 400);
}

#[tokio::test]
async fn a_denied_admin_sign_in_is_a_400_page() {
    let env = env().await;
    let (_, state) = env.admin_login_start().await;
    let reply = env
        .send(
            axum::http::Method::GET,
            &format!("/auth/discord/callback?error=access_denied&state={state}"),
            None,
            &[("cookie", &format!("admin_login={state}"))],
            Vec::new(),
            false,
        )
        .await;
    assert_eq!(reply.status, 400);
    assert!(reply.text().contains("cancelled"));
}

// ---- Dashboard access ----

#[tokio::test]
async fn without_a_valid_cookie_the_dashboard_is_the_login_page() {
    let env = env().await;
    let anonymous = env.get("/admin", None).await;
    assert_eq!(anonymous.status, 200);
    assert!(anonymous.text().contains("Sign in with Discord"));
    assert!(anonymous.text().contains("href=\"/admin/login\""));
    assert_hardened(&anonymous);

    let bogus = env
        .send(
            axum::http::Method::GET,
            "/admin",
            None,
            &[("cookie", "admin_session=bogus")],
            Vec::new(),
            false,
        )
        .await;
    assert!(bogus.text().contains("Sign in with Discord"));

    // A bearer token is not an admin credential.
    let admin_token = env.admin().await;
    let bearer = env.get("/admin", Some(&admin_token)).await;
    assert!(bearer.text().contains("Sign in with Discord"));
}

#[tokio::test]
async fn a_non_admin_session_gets_403() {
    let env = env().await;
    let token = env.user("rider", "555").await;
    let reply = env
        .send(
            axum::http::Method::GET,
            "/admin",
            None,
            &[("cookie", &cookie_header(&token))],
            Vec::new(),
            false,
        )
        .await;
    assert_eq!(reply.status, 403);
    assert_hardened(&reply);
}

#[tokio::test]
async fn an_admin_session_expires_after_twelve_hours() {
    let env = env().await;
    let browser = env.admin_browser().await;
    assert_eq!(env.admin_get("/admin", &browser).await.status, 200);
    env.advance(chrono::Duration::hours(12) + chrono::Duration::seconds(1));
    let after = env.admin_get("/admin", &browser).await;
    assert!(after.text().contains("Sign in with Discord"));
}

#[tokio::test]
async fn removing_an_admin_id_locks_the_session_out_at_once() {
    // Build a second environment whose admin list is empty over the same kind
    // of session: the service re-checks the list on every request.
    let env = env().await;
    let browser = env.admin_browser().await;
    assert_eq!(env.admin_get("/admin", &browser).await.status, 200);
    // The user row stays, but its Discord id no longer matches an admin id.
    sqlx::query("UPDATE users SET discord_id = '777'")
        .execute(env.store.pool())
        .await
        .unwrap();
    assert_eq!(env.admin_get("/admin", &browser).await.status, 403);
    let post = env
        .admin_post(
            "/admin/users/ban",
            &browser,
            &[("discord_id", "1"), ("reason", "x")],
        )
        .await;
    assert_eq!(post.status, 403);
}

// ---- Dashboard content ----

/// Every text input and textarea must have a unique id and a matching `<label for>`.
fn assert_every_text_field_is_labelled(html: &str) {
    let mut fields = 0;
    let mut ids = std::collections::HashSet::new();
    let starts = html
        .match_indices("<input type=\"text\"")
        .chain(html.match_indices("<textarea"));
    for (index, _) in starts {
        let tag = &html[index..index + html[index..].find('>').unwrap()];
        let id = tag
            .split(" id=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .unwrap_or_else(|| panic!("text field without id: {tag}"));
        assert!(ids.insert(id.to_string()), "duplicate id {id}");
        assert!(
            html.contains(&format!("<label class=\"visually-hidden\" for=\"{id}\"")),
            "no label for {id}"
        );
        fields += 1;
    }
    assert!(fields >= 4, "expected several text fields, found {fields}");
}

#[tokio::test]
async fn the_dashboard_lists_every_section_and_escapes_user_content() {
    let env = env().await;
    let ada = env.user("<img src=x onerror=1>", "101").await;
    let bob = env.user("bob", "102").await;
    let creator = env.user("maker", "103").await;
    let id = env
        .comment_id(
            &ada,
            "livery-pack",
            "<script>alert(1)</script> & \"quotes\"",
        )
        .await;
    assert_eq!(
        env.report(&bob, "livery-pack", &id, "spam").await.status,
        204
    );
    let hidden = env.comment_id(&ada, "livery-pack", "<b>hidden</b>").await;
    let moderator = env.moderation_admin().await;
    env.moderation
        .hide_comment(&moderator, &hidden, "<i>why</i>")
        .await
        .unwrap();
    let banned = env.user_row("bob").await;
    env.moderation
        .ban_user(&moderator, banned.id.as_str(), "<u>abuse</u>")
        .await
        .unwrap();
    env.publish_with_screenshot(&creator, "brazil-livery", "1.0", b"zipdata")
        .await;
    sqlx::query("UPDATE mod_versions SET title = '<script>t()</script>', description = 'd\"<x>'")
        .execute(env.store.pool())
        .await
        .unwrap();

    let browser = env.admin_browser().await;
    let page = env.admin_get("/admin", &browser).await;
    assert_eq!(page.status, 200);
    assert_hardened(&page);
    let html = page.text();
    for needle in [
        "id=\"reports\"",
        "id=\"pending\"",
        "id=\"hidden\"",
        "id=\"bans\"",
        "id=\"audit\"",
        "Hide comment",
        "Dismiss (spam)",
        "Ban author",
        "Approve",
        "Reject",
        "Restore",
        "Unban",
        "Ban by Discord id",
        "(Discord 101)",
        "1 report(s)",
        "/admin/mods/brazil-livery/versions/1.0/file",
        "<img src=\"/admin/mods/brazil-livery/versions/1.0/screenshots/0\"",
        "ban_user",
        "hide_comment",
    ] {
        assert!(html.contains(needle), "missing {needle}");
    }
    assert_every_text_field_is_labelled(&html);
    // Hostile content is escaped everywhere, never emitted raw.
    assert!(!html.contains("<script>"));
    assert!(!html.contains("<img src=x"));
    assert!(!html.contains("<b>hidden</b>"));
    assert!(!html.contains("<i>why</i>"));
    assert!(!html.contains("<u>abuse</u>"));
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;quotes&quot;"));
    assert!(html.contains("&lt;img src=x onerror=1&gt;"));
    assert!(html.contains("&lt;script&gt;t()&lt;/script&gt;"));
    // No JavaScript, one inline style block, every form carries the token.
    assert!(!html.contains("<script"));
    assert_eq!(html.matches("<style>").count(), 1);
    assert_eq!(
        html.matches("<form").count(),
        html.matches(&format!("name=\"csrf\" value=\"{}\"", browser.csrf))
            .count()
    );
}

#[tokio::test]
async fn an_empty_dashboard_says_so() {
    let env = env().await;
    let browser = env.admin_browser().await;
    let html = env.admin_get("/admin", &browser).await.text();
    assert!(html.contains("Nothing here."));
}

// ---- Actions ----

#[tokio::test]
async fn actions_without_or_with_a_wrong_csrf_token_change_nothing() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let bob = env.user("bob", "102").await;
    let id = env.comment_id(&ada, "livery-pack", "Hello").await;
    env.report(&bob, "livery-pack", &id, "spam").await;
    let browser = env.admin_browser().await;
    let uri = format!("/admin/comments/{id}/hide");

    // No token, a wrong token, an empty token, no cookie.
    let none = env
        .admin_post_form(&uri, Some(&browser.cookie), &[("reason", "x")])
        .await;
    assert_eq!(none.status, 403);
    assert_hardened(&none);
    for wrong in ["nope", ""] {
        let reply = env
            .admin_post_form(
                &uri,
                Some(&browser.cookie),
                &[("csrf", wrong), ("reason", "x")],
            )
            .await;
        assert_eq!(reply.status, 403);
    }
    let anonymous = env
        .admin_post_form(&uri, None, &[("csrf", &browser.csrf), ("reason", "x")])
        .await;
    assert_eq!(anonymous.status, 401);
    // A token minted for another session does not work either.
    let other = env
        .admin_post_form(
            &uri,
            Some(&browser.cookie),
            &[("csrf", "h:csrf:other"), ("reason", "x")],
        )
        .await;
    assert_eq!(other.status, 403);

    assert_eq!(env.visible_comments("livery-pack").await, ["Hello"]);
    assert_eq!(
        count(&env, "SELECT COUNT(*) FROM moderation_actions").await,
        0
    );
}

#[tokio::test]
async fn hiding_and_restoring_go_through_the_forms() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let bob = env.user("bob", "102").await;
    let id = env.comment_id(&ada, "livery-pack", "Buy now").await;
    env.report(&bob, "livery-pack", &id, "spam").await;
    let browser = env.admin_browser().await;

    let hide = env
        .admin_post(
            &format!("/admin/comments/{id}/hide"),
            &browser,
            &[("reason", "  advertising  ")],
        )
        .await;
    assert_eq!(hide.status, 303);
    assert_eq!(hide.header("location"), "/admin");
    assert!(env.visible_comments("livery-pack").await.is_empty());
    let admin = env.moderation_admin().await;
    assert!(env
        .moderation
        .open_reports(&admin)
        .await
        .unwrap()
        .is_empty());
    let audit = env.store.recent_audit(5).await.unwrap();
    assert_eq!(audit[0].reason.as_deref(), Some("advertising"));
    assert_eq!(audit[0].admin_name.as_deref(), Some("Mod"));

    let restore = env
        .admin_post(&format!("/admin/comments/{id}/restore"), &browser, &[])
        .await;
    assert_eq!(restore.status, 303);
    assert_eq!(env.visible_comments("livery-pack").await, ["Buy now"]);
}

#[tokio::test]
async fn dismissing_a_report_resolves_it() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let bob = env.user("bob", "102").await;
    let id = env.comment_id(&ada, "livery-pack", "Fine").await;
    env.report(&bob, "livery-pack", &id, "off_topic").await;
    let report_id: String = sqlx::query_scalar("SELECT id FROM comment_reports")
        .fetch_one(env.store.pool())
        .await
        .unwrap();
    let browser = env.admin_browser().await;
    let reply = env
        .admin_post(
            &format!("/admin/reports/{report_id}/dismiss"),
            &browser,
            &[],
        )
        .await;
    assert_eq!(reply.status, 303);
    let admin = env.moderation_admin().await;
    assert!(env
        .moderation
        .open_reports(&admin)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(env.visible_comments("livery-pack").await, ["Fine"]);
    let missing = env
        .admin_post("/admin/reports/nope/dismiss", &browser, &[])
        .await;
    assert_eq!(missing.status, 404);
}

#[tokio::test]
async fn banning_by_user_id_by_discord_id_and_unbanning() {
    let env = env().await;
    let ada = env.user("ada", "101").await;
    let _bob = env.user("bob", "102").await;
    let browser = env.admin_browser().await;

    let by_id = env
        .admin_post("/admin/users/user-ada/ban", &browser, &[("reason", "spam")])
        .await;
    assert_eq!(by_id.status, 303);
    assert_eq!(env.comment(&ada, "livery-pack", "hi").await.status, 403);

    let by_discord = env
        .admin_post(
            "/admin/users/ban",
            &browser,
            &[("discord_id", " 102 "), ("reason", "abuse")],
        )
        .await;
    assert_eq!(by_discord.status, 303);
    assert_eq!(count(&env, "SELECT COUNT(*) FROM user_bans").await, 2);

    let unban = env
        .admin_post("/admin/users/user-ada/unban", &browser, &[])
        .await;
    assert_eq!(unban.status, 303);
    assert_eq!(env.comment(&ada, "livery-pack", "hi").await.status, 201);
    assert_eq!(
        env.admin_post("/admin/users/user-ada/unban", &browser, &[])
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn bad_form_input_renders_an_error_page_without_changing_state() {
    let env = env().await;
    let _ada = env.user("ada", "101").await;
    let browser = env.admin_browser().await;
    // A missing reason, an unknown Discord id, banning yourself and another admin.
    let empty = env
        .admin_post("/admin/users/user-ada/ban", &browser, &[("reason", "  ")])
        .await;
    assert_eq!(empty.status, 400);
    assert!(empty.text().contains("<html"));
    assert_hardened(&empty);
    let unknown = env
        .admin_post(
            "/admin/users/ban",
            &browser,
            &[("discord_id", "424242"), ("reason", "x")],
        )
        .await;
    assert_eq!(unknown.status, 400);
    let myself = env
        .admin_post(
            "/admin/users/ban",
            &browser,
            &[("discord_id", ADMIN_DISCORD_ID), ("reason", "x")],
        )
        .await;
    assert_eq!(myself.status, 400);
    let missing_user = env
        .admin_post("/admin/users/nobody/ban", &browser, &[("reason", "x")])
        .await;
    assert_eq!(missing_user.status, 404);
    // A body that is not a form.
    let garbage = env
        .send(
            axum::http::Method::POST,
            "/admin/users/user-ada/ban",
            None,
            &[("cookie", &browser.cookie), ("content-type", "text/plain")],
            b"x".to_vec(),
            false,
        )
        .await;
    assert_eq!(garbage.status, 400);
    assert_eq!(count(&env, "SELECT COUNT(*) FROM user_bans").await, 0);
}

#[tokio::test]
async fn reviewing_a_mod_from_the_page_approves_and_rejects_with_audit() {
    let env = env().await;
    let creator = env.user("maker", "103").await;
    env.publish_with_screenshot(&creator, "brazil-livery", "1.0", b"zipdata")
        .await;
    env.publish_with_screenshot(&creator, "brazil-livery", "2.0", b"zipdata2")
        .await;
    let browser = env.admin_browser().await;

    // CSRF first: nothing is reviewed without the token.
    let no_csrf = env
        .admin_post_form(
            "/admin/review/brazil-livery/1.0/approve",
            Some(&browser.cookie),
            &[],
        )
        .await;
    assert_eq!(no_csrf.status, 403);
    assert_eq!(
        count(
            &env,
            "SELECT COUNT(*) FROM mod_versions WHERE status = 'pending'"
        )
        .await,
        2
    );

    let approve = env
        .admin_post("/admin/review/brazil-livery/1.0/approve", &browser, &[])
        .await;
    assert_eq!(approve.status, 303);
    let blank = env
        .admin_post(
            "/admin/review/brazil-livery/2.0/reject",
            &browser,
            &[("reason", " ")],
        )
        .await;
    assert_eq!(blank.status, 400);
    let reject = env
        .admin_post(
            "/admin/review/brazil-livery/2.0/reject",
            &browser,
            &[("reason", "wrong game")],
        )
        .await;
    assert_eq!(reject.status, 303);

    let catalog = env.get("/catalog", None).await.json();
    assert_eq!(catalog["entries"][0]["version"], "1.0");
    let audit = env.store.recent_audit(5).await.unwrap();
    let actions: Vec<(&str, Option<&str>)> = audit
        .iter()
        .map(|e| (e.action.as_str(), e.reason.as_deref()))
        .collect();
    assert_eq!(
        actions,
        [
            ("reject_version", Some("wrong game")),
            ("approve_version", None)
        ]
    );
    // Reviewing something that is not pending is a 400 page.
    let again = env
        .admin_post("/admin/review/brazil-livery/1.0/approve", &browser, &[])
        .await;
    assert_eq!(again.status, 400);
}

#[tokio::test]
async fn the_json_review_routes_still_work_with_a_bearer_token() {
    let env = env().await;
    let creator = env.user("maker", "103").await;
    env.publish(&creator, "brazil-livery", "1.0", b"zipdata", 4)
        .await;
    let admin = env.admin().await;
    assert_eq!(
        env.get("/admin/mods/pending", Some(&admin)).await.status,
        200
    );
    assert_eq!(
        env.approve(&admin, "brazil-livery", "1.0").await.status,
        200
    );
}

#[tokio::test]
async fn logging_out_revokes_the_session_and_clears_the_cookie() {
    let env = env().await;
    let browser = env.admin_browser().await;
    let denied = env
        .admin_post_form("/admin/logout", Some(&browser.cookie), &[])
        .await;
    assert_eq!(denied.status, 403);
    let reply = env.admin_post("/admin/logout", &browser, &[]).await;
    assert_eq!(reply.status, 303);
    let cleared = reply
        .set_cookies()
        .into_iter()
        .find(|c| c.starts_with("admin_session="))
        .unwrap();
    assert!(cleared.contains("Max-Age=0") && cleared.contains("Path=/admin"));
    let after = env.admin_get("/admin", &browser).await;
    assert!(after.text().contains("Sign in with Discord"));
}

#[tokio::test]
async fn the_admin_is_rate_limited_per_user() {
    // 120 requests per minute per admin: the next one is a 429 page.
    let env = env().await;
    let browser = env.admin_browser().await;
    let mut last = 0;
    for _ in 0..130 {
        last = env.admin_get("/admin", &browser).await.status;
        if last == 429 {
            break;
        }
    }
    assert_eq!(last, 429);
}

// ---- Review files ----

#[tokio::test]
async fn the_file_route_serves_pending_and_approved_files_as_attachments() {
    let env = env().await;
    let creator = env.user("maker", "103").await;
    env.publish_with_screenshot(&creator, "brazil-livery", "1.0", b"zipdata")
        .await;
    let browser = env.admin_browser().await;
    let uri = "/admin/mods/brazil-livery/versions/1.0/file";

    let pending = env.admin_get(uri, &browser).await;
    assert_eq!(pending.status, 200);
    assert_eq!(pending.bytes, b"zipdata");
    assert_eq!(
        pending.header("content-disposition"),
        "attachment; filename=\"brazil-livery.zip\""
    );
    assert_eq!(pending.header("content-type"), "application/octet-stream");
    assert_hardened(&pending);

    let partial = env
        .send(
            axum::http::Method::GET,
            uri,
            None,
            &[("cookie", &browser.cookie), ("range", "bytes=0-2")],
            Vec::new(),
            false,
        )
        .await;
    assert_eq!(partial.status, 206);
    assert_eq!(partial.bytes, b"zip");

    env.admin_post("/admin/review/brazil-livery/1.0/approve", &browser, &[])
        .await;
    let approved = env.admin_get(uri, &browser).await;
    assert_eq!(approved.status, 200);
    assert_eq!(approved.bytes, b"zipdata");

    assert_eq!(
        env.admin_get("/admin/mods/brazil-livery/versions/9.9/file", &browser)
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn screenshots_are_served_with_their_type_and_nosniff() {
    let env = env().await;
    let creator = env.user("maker", "103").await;
    env.publish_with_screenshot(&creator, "brazil-livery", "1.0", b"zipdata")
        .await;
    let browser = env.admin_browser().await;
    let reply = env
        .admin_get(
            "/admin/mods/brazil-livery/versions/1.0/screenshots/0",
            &browser,
        )
        .await;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.bytes, PNG);
    assert_eq!(reply.header("content-type"), "image/png");
    assert_hardened(&reply);
    for uri in [
        "/admin/mods/brazil-livery/versions/1.0/screenshots/7",
        "/admin/mods/brazil-livery/versions/1.0/screenshots/x",
        "/admin/mods/nope/versions/1.0/screenshots/0",
    ] {
        assert_eq!(env.admin_get(uri, &browser).await.status, 404, "{uri}");
    }
}

#[tokio::test]
async fn review_files_are_admin_only() {
    let env = env().await;
    let creator = env.user("maker", "103").await;
    env.publish_with_screenshot(&creator, "brazil-livery", "1.0", b"zipdata")
        .await;
    for uri in [
        "/admin/mods/brazil-livery/versions/1.0/file",
        "/admin/mods/brazil-livery/versions/1.0/screenshots/0",
    ] {
        // Anonymous, a bearer token and a non-admin session cookie.
        assert_eq!(env.get(uri, None).await.status, 401, "{uri}");
        assert_eq!(env.get(uri, Some(&creator)).await.status, 401, "{uri}");
        let non_admin = env
            .send(
                axum::http::Method::GET,
                uri,
                None,
                &[("cookie", &cookie_header(&creator))],
                Vec::new(),
                false,
            )
            .await;
        assert_eq!(non_admin.status, 403, "{uri}");
    }
}

#[allow(dead_code)]
fn _types(_: AdminBrowser) {}
