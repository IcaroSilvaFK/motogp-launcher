//! The admin web: server-rendered HTML under `/admin` (no JavaScript).
//!
//! # Sessions and CSRF
//!
//! `GET /admin/login` starts an admin-purpose Discord sign-in and sets the
//! short-lived `admin_login` cookie (login CSRF: the callback needs it to equal
//! the OAuth `state`). The callback (`/auth/discord/callback`, see `auth.rs`)
//! then opens a 12 h session and sets `admin_session` (HttpOnly, Secure,
//! SameSite=Strict, Path=/admin). Every admin request is re-checked against
//! `ADMIN_DISCORD_IDS`, so removing an id locks the admin out at once. Only the
//! cookie is accepted here, never a bearer token. Every POST carries a hidden
//! `csrf` field, `hash("csrf:" + session token)`, compared in constant time.
//!
//! # Form routes
//!
//! The JSON `/admin/mods/{mod}/versions/{v}/approve|reject` routes (bearer, for
//! curl) stay as they are, so the HTML forms post to `/admin/review/{mod}/{v}/…`.
//!
//! Every response sends the headers of [`harden`]; all user content is
//! HTML-escaped by [`esc`].

use std::path::Path as FsPath;

use axum::body::Body;
use axum::extract::rejection::FormRejection;
use axum::extract::{FromRequestParts, Path, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE, COOKIE, LOCATION, RANGE, SET_COOKIE,
};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use chrono::{DateTime, SecondsFormat, Utc};
use community_application::auth::AdminLogin;
use community_application::AppError;
use community_domain::mods::ModVersion;
use community_domain::User;
use serde::Deserialize;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::admin_css::ADMIN_CSS;
use crate::error::status_of;
use crate::mods::{encode_segment, escape_html as esc};
use crate::AppState;

const SESSION_COOKIE: &str = "admin_session";
const LOGIN_COOKIE: &str = "admin_login";
const LOGIN_COOKIE_MAX_AGE_SECS: i64 = 600;
const AUDIT_LIMIT: u32 = 50;
const CSP: &str = "default-src 'none'; img-src 'self'; style-src 'unsafe-inline'; \
                   form-action 'self'; frame-ancestors 'none'; base-uri 'none'";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin", get(dashboard))
        .route("/admin/login", get(login))
        .route("/admin/logout", post(logout))
        .route("/admin/comments/{id}/hide", post(hide))
        .route("/admin/comments/{id}/restore", post(restore))
        .route("/admin/reports/{id}/dismiss", post(dismiss))
        .route("/admin/users/ban", post(ban_by_discord_id))
        .route("/admin/users/{user_id}/ban", post(ban))
        .route("/admin/users/{user_id}/unban", post(unban))
        .route("/admin/review/{mod_id}/{version}/approve", post(approve))
        .route("/admin/review/{mod_id}/{version}/reject", post(reject))
        .route("/admin/mods/{mod_id}/versions/{version}/file", get(file))
        .route(
            "/admin/mods/{mod_id}/versions/{version}/screenshots/{index}",
            get(screenshot),
        )
}

// ---- Responses ----

/// Adds the security headers every admin response carries.
pub fn harden(response: &mut Response) {
    let headers = response.headers_mut();
    for (name, value) in [
        ("cache-control", "no-store"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        ("x-content-type-options", "nosniff"),
        ("content-security-policy", CSP),
    ] {
        headers.insert(name, HeaderValue::from_static(value));
    }
}

fn page(status: StatusCode, title: &str, body: &str) -> Response {
    page_with_head(status, title, "", body)
}

fn page_with_head(status: StatusCode, title: &str, head: &str, body: &str) -> Response {
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{title} - MotoGP community admin</title>{head}<style>{ADMIN_CSS}</style></head>\
         <body class=\"admin\">{body}</body></html>",
        title = esc(title),
    );
    let mut response = (status, html).into_response();
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    harden(&mut response);
    response
}

/// A page with a message and a link (login, errors).
fn notice(status: StatusCode, title: &str, class: &str, message: &str, link: &str) -> Response {
    page(
        status,
        title,
        &format!(
            "<main class=\"{class}\"><h1>{}</h1><p>{}</p>{link}</main>",
            esc(title),
            esc(message),
        ),
    )
}

fn login_page() -> Response {
    notice(
        StatusCode::OK,
        "Sign in",
        "notice",
        "Moderators sign in with Discord.",
        "<p><a class=\"button primary\" href=\"/admin/login\">Sign in with Discord</a></p>",
    )
}

fn see_other_to_admin() -> Response {
    let mut response = StatusCode::SEE_OTHER.into_response();
    response
        .headers_mut()
        .insert(LOCATION, HeaderValue::from_static("/admin"));
    harden(&mut response);
    response
}

fn add_cookie(response: &mut Response, cookie: &str) {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        response.headers_mut().append(SET_COOKIE, value);
    }
}

fn session_cookie(token: &str, max_age_secs: i64) -> String {
    format!(
        "{SESSION_COOKIE}={token}; HttpOnly; Secure; SameSite=Strict; Path=/admin; \
         Max-Age={max_age_secs}"
    )
}

fn login_cookie(value: &str, max_age_secs: i64) -> String {
    format!(
        "{LOGIN_COOKIE}={value}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={max_age_secs}"
    )
}

/// The value of the named cookie, if any `Cookie` header carries it.
pub fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find_map(|(key, value)| (key == name && !value.is_empty()).then_some(value))
}

/// Why an admin request failed. Always rendered as an HTML page.
pub enum AdminReject {
    App(AppError),
    Csrf,
}

impl From<AppError> for AdminReject {
    fn from(error: AppError) -> Self {
        Self::App(error)
    }
}

impl IntoResponse for AdminReject {
    fn into_response(self) -> Response {
        match self {
            Self::Csrf => notice(
                StatusCode::FORBIDDEN,
                "Form expired",
                "error",
                "The form is missing its security token or it is wrong. \
                 Nothing was changed. Go back, reload the page and try again.",
                "<p><a href=\"/admin\">Back to the admin page</a></p>",
            ),
            Self::App(error) => error_page(&error),
        }
    }
}

/// Renders an application error as a simple HTML page with the right status.
pub fn error_page(error: &AppError) -> Response {
    if let AppError::Internal(detail) = error {
        tracing::error!(detail = %detail, "internal error");
    }
    let status = status_of(error);
    let link = if matches!(error, AppError::Unauthorized) {
        "<p><a href=\"/admin/login\">Sign in with Discord</a></p>"
    } else {
        "<p><a href=\"/admin\">Back to the admin page</a></p>"
    };
    let mut response = notice(
        status,
        &format!("Error {}", status.as_u16()),
        "error",
        &error.public_message(),
        link,
    );
    if let AppError::RateLimited { retry_after_secs } = error {
        if let Ok(value) = HeaderValue::from_str(&retry_after_secs.to_string()) {
            response.headers_mut().insert("retry-after", value);
        }
    }
    response
}

// ---- Session extractor ----

/// A verified admin session (cookie only).
pub struct AdminSession {
    pub user: User,
    token: String,
    csrf: String,
}

impl AdminSession {
    /// Fails with 403 unless the submitted token matches this session.
    fn check_csrf(&self, app: &AppState, submitted: &str) -> Result<(), AdminReject> {
        if app.auth.verify_csrf(&self.token, submitted) {
            Ok(())
        } else {
            Err(AdminReject::Csrf)
        }
    }
}

impl FromRequestParts<AppState> for AdminSession {
    type Rejection = AdminReject;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = cookie_value(&parts.headers, SESSION_COOKIE).ok_or(AppError::Unauthorized)?;
        let user = state
            .auth
            .authenticate_admin(token, &state.moderation.admin_discord_ids)
            .await?;
        state.check_user_limit(&user.id)?;
        Ok(Self {
            csrf: state.auth.csrf_token(token),
            token: token.to_owned(),
            user,
        })
    }
}

// ---- Login and callback ----

/// `GET /admin/login`: 302 to Discord with the `admin_login` cookie.
async fn login(State(app): State<AppState>) -> Result<Response, AdminReject> {
    let (state, url) = app.auth.start_admin().await?;
    let location = HeaderValue::from_str(&url)
        .map_err(|_| AppError::Internal("redirect is not a valid header value".into()))?;
    let mut response = StatusCode::FOUND.into_response();
    response.headers_mut().insert(LOCATION, location);
    add_cookie(
        &mut response,
        &login_cookie(&state, LOGIN_COOKIE_MAX_AGE_SECS),
    );
    harden(&mut response);
    Ok(response)
}

/// Finishes an admin sign-in (called by the shared Discord callback): an admin
/// gets the session cookie and a 200 page that forwards to `/admin` (a Strict
/// cookie is not sent on a redirect chain that started cross-site, so a 302
/// would land on the login page); anyone else gets a 403 page.
pub async fn finish_login(app: &AppState, user: User) -> Result<Response, AppError> {
    let login = app
        .auth
        .open_admin_session(user, &app.moderation.admin_discord_ids)
        .await?;
    let mut response = match login {
        AdminLogin::Granted(issued) => {
            let mut ok = page_with_head(
                StatusCode::OK,
                "Signed in",
                "<meta http-equiv=\"refresh\" content=\"0;url=/admin\">",
                "<main class=\"notice\"><h1>Signed in</h1>\
                 <p><a href=\"/admin\">Continue to the admin page</a></p></main>",
            );
            add_cookie(&mut ok, &session_cookie(issued.token.expose(), 12 * 3600));
            ok
        }
        AdminLogin::NotAdmin(_) => notice(
            StatusCode::FORBIDDEN,
            "Not allowed",
            "error",
            "This Discord account is not an administrator of the community server.",
            "",
        ),
    };
    add_cookie(&mut response, &login_cookie("", 0));
    Ok(response)
}

// ---- Dashboard ----

fn timestamp(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn csrf_field(csrf: &str) -> String {
    format!(
        "<input type=\"hidden\" name=\"csrf\" value=\"{}\">",
        esc(csrf)
    )
}

/// A one-button POST form; `reason` adds a required text field.
fn action_form(
    action: &str,
    csrf: &str,
    label: &str,
    class: &str,
    reason_placeholder: Option<&str>,
) -> String {
    let reason = reason_placeholder.map_or_else(String::new, |placeholder| {
        // The action URL is unique per row, so it makes a unique id.
        let id: String = format!("reason{action}")
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        format!(
            "<label class=\"visually-hidden\" for=\"{id}\">{p}</label>\
             <input type=\"text\" id=\"{id}\" name=\"reason\" maxlength=\"500\" required placeholder=\"{p}\">",
            p = esc(placeholder)
        )
    });
    format!(
        "<form class=\"admin-form\" method=\"post\" action=\"{}\">{}{reason}\
         <button type=\"submit\" class=\"{class}\">{}</button></form>",
        esc(action),
        csrf_field(csrf),
        esc(label),
    )
}

fn section(id: &str, title: &str, count: usize, content: &str) -> String {
    let body = if count == 0 {
        "<p class=\"muted\">Nothing here.</p>"
    } else {
        content
    };
    format!(
        "<section class=\"admin-section\" id=\"{id}\"><h2>{} <span class=\"badge\">{count}</span></h2>{body}</section>",
        esc(title)
    )
}

fn user_label(user: &User) -> String {
    format!(
        "{} <span class=\"muted\">(Discord {})</span>",
        esc(user.display_name()),
        esc(user.discord_id.as_str())
    )
}

fn version_card(version: &ModVersion, csrf: &str) -> String {
    let mod_id = encode_segment(version.mod_id.as_str());
    let ver = encode_segment(&version.version);
    let base = format!("/admin/mods/{mod_id}/versions/{ver}");
    let thumbs: String = version
        .screenshots
        .iter()
        .map(|shot| {
            format!(
                "<a href=\"{base}/screenshots/{0}\"><img src=\"{base}/screenshots/{0}\" \
                 alt=\"Screenshot {1}\" loading=\"lazy\"></a>",
                shot.index,
                shot.index + 1
            )
        })
        .collect();
    let long = version
        .long_description
        .as_deref()
        .map_or_else(String::new, |text| {
            format!("<pre class=\"long-description\">{}</pre>", esc(text))
        });
    format!(
        "<article class=\"admin-card version\"><h3>{title} <span class=\"muted\">{mod_slug} {version_no}</span></h3>\
         <table class=\"admin-table\"><tbody>\
         <tr><th>Game</th><td>{game}</td></tr>\
         <tr><th>Category</th><td>{category}</td></tr>\
         <tr><th>Description</th><td>{description}</td></tr>\
         <tr><th>File</th><td>{file_name} ({size} bytes) - SHA-256 <code>{sha}</code> \
         - <a href=\"{base}/file\">Download</a></td></tr>\
         <tr><th>Uploaded</th><td>{uploaded}</td></tr></tbody></table>{long}\
         <div class=\"thumbs\">{thumbs}</div>{approve}{reject}</article>",
        title = esc(&version.title),
        mod_slug = esc(version.mod_id.as_str()),
        version_no = esc(&version.version),
        game = esc(&version.game),
        category = esc(&version.category),
        description = esc(&version.description),
        file_name = esc(&version.file_name),
        size = version.size,
        sha = esc(&version.sha256),
        uploaded = timestamp(version.uploaded_at),
        approve = action_form(
            &format!("/admin/review/{mod_id}/{ver}/approve"),
            csrf,
            "Approve",
            "primary",
            None
        ),
        reject = action_form(
            &format!("/admin/review/{mod_id}/{ver}/reject"),
            csrf,
            "Reject",
            "danger",
            Some("Reason shown to the creator"),
        ),
    )
}

async fn dashboard(
    State(app): State<AppState>,
    session: Result<AdminSession, AdminReject>,
) -> Result<Response, AdminReject> {
    let session = match session {
        Ok(session) => session,
        Err(AdminReject::App(AppError::Unauthorized)) => return Ok(login_page()),
        Err(other) => return Err(other),
    };
    let admin = &session.user;
    let csrf = &session.csrf;
    let reports = app.moderation.open_reports(admin).await?;
    let pending = app.mods.service.list_pending(admin).await?;
    let hidden = app.moderation.hidden_comments(admin).await?;
    let bans = app.moderation.bans(admin).await?;
    let audit = app.moderation.recent_audit(admin, AUDIT_LIMIT).await?;

    let reports_html: String = reports
        .iter()
        .map(|group| {
            let comment = &group.comment;
            let reasons: Vec<&str> = group.reasons().iter().map(|r| r.as_str()).collect();
            let details: String = group
                .reports
                .iter()
                .filter_map(|r| r.details.as_deref())
                .map(|d| format!("<li>{}</li>", esc(d)))
                .collect();
            let dismiss: String = group
                .reports
                .iter()
                .map(|r| {
                    action_form(
                        &format!("/admin/reports/{}/dismiss", encode_segment(&r.id)),
                        csrf,
                        &format!("Dismiss ({})", r.reason.as_str()),
                        "",
                        None,
                    )
                })
                .collect();
            let comment_id = encode_segment(comment.comment.id.as_str());
            let author_id = encode_segment(comment.author.id.as_str());
            format!(
                "<article class=\"admin-card report\"><p>{author} on <strong>{mod_id}</strong> \
                 <span class=\"muted\">{created}</span></p>\
                 <pre class=\"comment-body\">{body}</pre>\
                 <p><span class=\"badge\">{count} report(s)</span> {reasons}</p>\
                 <ul class=\"report-details\">{details}</ul>{hide}{dismiss}{ban}</article>",
                author = user_label(&comment.author),
                mod_id = esc(comment.comment.mod_id.as_str()),
                created = timestamp(comment.comment.created_at),
                body = esc(comment.comment.body.as_str()),
                count = group.report_count(),
                reasons = esc(&reasons.join(", ")),
                hide = action_form(
                    &format!("/admin/comments/{comment_id}/hide"),
                    csrf,
                    "Hide comment",
                    "danger",
                    Some("Reason"),
                ),
                ban = action_form(
                    &format!("/admin/users/{author_id}/ban"),
                    csrf,
                    "Ban author",
                    "danger",
                    Some("Ban reason"),
                ),
            )
        })
        .collect();

    let pending_html: String = pending.iter().map(|v| version_card(v, csrf)).collect();

    let hidden_html: String = hidden
        .iter()
        .map(|item| {
            let comment = &item.comment;
            format!(
                "<article class=\"admin-card hidden-comment\"><p>{author} on <strong>{mod_id}</strong> \
                 <span class=\"muted\">hidden {when}: {reason}</span></p>\
                 <pre class=\"comment-body\">{body}</pre>{restore}</article>",
                author = user_label(&comment.author),
                mod_id = esc(comment.comment.mod_id.as_str()),
                when = timestamp(item.hidden_at),
                reason = esc(item.reason.as_deref().unwrap_or("-")),
                body = esc(comment.comment.body.as_str()),
                restore = action_form(
                    &format!(
                        "/admin/comments/{}/restore",
                        encode_segment(comment.comment.id.as_str())
                    ),
                    csrf,
                    "Restore",
                    "primary",
                    None,
                ),
            )
        })
        .collect();

    let bans_rows: String = bans
        .iter()
        .map(|item| {
            format!(
                "<tr><td>{user}</td><td>{reason}</td><td>{when}</td><td>{unban}</td></tr>",
                user = user_label(&item.user),
                reason = esc(&item.ban.reason),
                when = timestamp(item.ban.banned_at),
                unban = action_form(
                    &format!(
                        "/admin/users/{}/unban",
                        encode_segment(item.user.id.as_str())
                    ),
                    csrf,
                    "Unban",
                    "primary",
                    None,
                ),
            )
        })
        .collect();
    let bans_table = format!(
        "<table class=\"admin-table\"><thead><tr><th>User</th><th>Reason</th><th>Since</th><th></th></tr></thead>\
         <tbody>{bans_rows}</tbody></table>"
    );
    let ban_form = format!(
        "<form class=\"admin-form ban-by-id\" method=\"post\" action=\"/admin/users/ban\">{}\
         <label class=\"visually-hidden\" for=\"ban-discord-id\">Discord id</label>\
         <input type=\"text\" id=\"ban-discord-id\" name=\"discord_id\" inputmode=\"numeric\" maxlength=\"20\" required placeholder=\"Discord id\">\
         <label class=\"visually-hidden\" for=\"ban-discord-reason\">Ban reason</label>\
         <input type=\"text\" id=\"ban-discord-reason\" name=\"reason\" maxlength=\"500\" required placeholder=\"Ban reason\">\
         <button type=\"submit\" class=\"danger\">Ban by Discord id</button></form>",
        csrf_field(csrf)
    );

    let audit_rows: String = audit
        .iter()
        .map(|entry| {
            format!(
                "<tr><td>{when}</td><td>{admin}</td><td>{action}</td><td>{target}</td><td>{reason}</td></tr>",
                when = timestamp(entry.created_at),
                admin = esc(entry.admin_name.as_deref().unwrap_or("(deleted user)")),
                action = esc(entry.action.as_str()),
                target = esc(&entry.target),
                reason = esc(entry.reason.as_deref().unwrap_or("")),
            )
        })
        .collect();
    let audit_table = format!(
        "<table class=\"admin-table\"><thead><tr><th>When</th><th>Admin</th><th>Action</th><th>Target</th><th>Reason</th></tr></thead>\
         <tbody>{audit_rows}</tbody></table>"
    );

    let body = format!(
        "<header class=\"admin-header\"><h1>MotoGP community admin</h1>\
         <div>{name} {logout}</div></header><main class=\"admin-main\">{reports}{pending}{hidden}{bans}{audit}</main>",
        name = esc(admin.display_name()),
        logout = action_form("/admin/logout", csrf, "Sign out", "", None),
        reports = section("reports", "Open reports", reports.len(), &reports_html),
        pending = section("pending", "Versions waiting for review", pending.len(), &pending_html),
        hidden = section("hidden", "Hidden comments", hidden.len(), &hidden_html),
        bans = section(
            "bans",
            "Banned users",
            bans.len(),
            &bans_table
        )
        .replace("</section>", &format!("{ban_form}</section>")),
        audit = section("audit", "Recent actions", audit.len(), &audit_table),
    );
    Ok(page(StatusCode::OK, "Dashboard", &body))
}

// ---- Actions ----

#[derive(Deserialize, Default)]
struct ActionForm {
    #[serde(default)]
    csrf: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    discord_id: String,
}

type FormResult = Result<Form<ActionForm>, FormRejection>;

fn parse(form: FormResult) -> Result<ActionForm, AdminReject> {
    form.map(|Form(form)| form)
        .map_err(|_| AppError::BadRequest("The form is invalid.".into()).into())
}

async fn hide(
    State(app): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
    form: FormResult,
) -> Result<Response, AdminReject> {
    let form = parse(form)?;
    session.check_csrf(&app, &form.csrf)?;
    app.moderation
        .hide_comment(&session.user, &id, &form.reason)
        .await?;
    Ok(see_other_to_admin())
}

async fn restore(
    State(app): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
    form: FormResult,
) -> Result<Response, AdminReject> {
    let form = parse(form)?;
    session.check_csrf(&app, &form.csrf)?;
    app.moderation.restore_comment(&session.user, &id).await?;
    Ok(see_other_to_admin())
}

async fn dismiss(
    State(app): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
    form: FormResult,
) -> Result<Response, AdminReject> {
    let form = parse(form)?;
    session.check_csrf(&app, &form.csrf)?;
    app.moderation.dismiss_report(&session.user, &id).await?;
    Ok(see_other_to_admin())
}

async fn ban(
    State(app): State<AppState>,
    session: AdminSession,
    Path(user_id): Path<String>,
    form: FormResult,
) -> Result<Response, AdminReject> {
    let form = parse(form)?;
    session.check_csrf(&app, &form.csrf)?;
    app.moderation
        .ban_user(&session.user, &user_id, &form.reason)
        .await?;
    Ok(see_other_to_admin())
}

async fn ban_by_discord_id(
    State(app): State<AppState>,
    session: AdminSession,
    form: FormResult,
) -> Result<Response, AdminReject> {
    let form = parse(form)?;
    session.check_csrf(&app, &form.csrf)?;
    app.moderation
        .ban_by_discord_id(&session.user, form.discord_id.trim(), &form.reason)
        .await?;
    Ok(see_other_to_admin())
}

async fn unban(
    State(app): State<AppState>,
    session: AdminSession,
    Path(user_id): Path<String>,
    form: FormResult,
) -> Result<Response, AdminReject> {
    let form = parse(form)?;
    session.check_csrf(&app, &form.csrf)?;
    app.moderation.unban(&session.user, &user_id).await?;
    Ok(see_other_to_admin())
}

async fn approve(
    State(app): State<AppState>,
    session: AdminSession,
    Path((mod_id, version)): Path<(String, String)>,
    form: FormResult,
) -> Result<Response, AdminReject> {
    let form = parse(form)?;
    session.check_csrf(&app, &form.csrf)?;
    app.mods
        .service
        .approve(&session.user, &mod_id, &version)
        .await?;
    Ok(see_other_to_admin())
}

async fn reject(
    State(app): State<AppState>,
    session: AdminSession,
    Path((mod_id, version)): Path<(String, String)>,
    form: FormResult,
) -> Result<Response, AdminReject> {
    let form = parse(form)?;
    session.check_csrf(&app, &form.csrf)?;
    app.mods
        .service
        .reject(&session.user, &mod_id, &version, &form.reason)
        .await?;
    Ok(see_other_to_admin())
}

async fn logout(
    State(app): State<AppState>,
    session: AdminSession,
    form: FormResult,
) -> Result<Response, AdminReject> {
    let form = parse(form)?;
    session.check_csrf(&app, &form.csrf)?;
    app.auth.logout(&session.token).await?;
    let mut response = see_other_to_admin();
    add_cookie(&mut response, &session_cookie("", 0));
    Ok(response)
}

// ---- Review files ----

/// A file name that is safe inside `Content-Disposition: filename="..."`.
fn safe_download_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ' | '(' | ')' | '+') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "download".into()
    } else {
        cleaned
    }
}

/// `GET /admin/mods/{mod}/versions/{v}/file`: the stored file as an attachment
/// (pending or approved), with `Range` support.
async fn file(
    State(app): State<AppState>,
    session: AdminSession,
    Path((mod_id, version)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, AdminReject> {
    let (found, path) = app
        .mods
        .service
        .review_file(&session.user, &mod_id, &version)
        .await?;
    let mut request = Request::builder().uri("/");
    if let Some(range) = headers.get(RANGE) {
        request = request.header(RANGE, range);
    }
    let request = request
        .body(Body::empty())
        .map_err(|_| AppError::Internal("could not build the file request".into()))?;
    let served = ServeFile::new(FsPath::new(&path))
        .oneshot(request)
        .await
        .map_err(|_| AppError::Internal("could not read the file".into()))?;
    let mut response = served.map(Body::new);
    let disposition = format!(
        "attachment; filename=\"{}\"",
        safe_download_name(&found.file_name)
    );
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(value) = HeaderValue::from_str(&disposition) {
        headers.insert(CONTENT_DISPOSITION, value);
    }
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    harden(&mut response);
    Ok(response)
}

/// `GET /admin/mods/{mod}/versions/{v}/screenshots/{n}`: the image with the
/// content type that was sniffed from its bytes at upload.
async fn screenshot(
    State(app): State<AppState>,
    session: AdminSession,
    Path((mod_id, version, index)): Path<(String, String, String)>,
) -> Result<Response, AdminReject> {
    let index: usize = index.parse().map_err(|_| AppError::NotFound)?;
    let (shot, path) = app
        .mods
        .service
        .review_screenshot(&session.user, &mod_id, &version, index)
        .await?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| AppError::NotFound)?;
    // Only raster images are ever shown inline.
    let content_type = match shot.content_type.as_str() {
        "image/png" => "image/png",
        "image/jpeg" => "image/jpeg",
        "image/webp" => "image/webp",
        "image/gif" => "image/gif",
        _ => "application/octet-stream",
    };
    let mut response = bytes.into_response();
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    harden(&mut response);
    Ok(response)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn cookies_are_found_by_name_across_headers() {
        let mut headers = HeaderMap::new();
        headers.append(COOKIE, HeaderValue::from_static("a=1; admin_session=tok"));
        headers.append(COOKIE, HeaderValue::from_static("b=2"));
        assert_eq!(cookie_value(&headers, "admin_session"), Some("tok"));
        assert_eq!(cookie_value(&headers, "b"), Some("2"));
        assert_eq!(cookie_value(&headers, "missing"), None);
        // A longer name is not a match, and an empty value counts as missing.
        headers.append(
            COOKIE,
            HeaderValue::from_static("xadmin_session=bad; empty="),
        );
        assert_eq!(cookie_value(&headers, "admin_session"), Some("tok"));
        assert_eq!(cookie_value(&headers, "empty"), None);
    }

    #[test]
    fn cookie_attributes_match_the_spec() {
        let session = session_cookie("tok", 43_200);
        for part in [
            "admin_session=tok",
            "HttpOnly",
            "Secure",
            "SameSite=Strict",
            "Path=/admin",
            "Max-Age=43200",
        ] {
            assert!(session.contains(part), "{session}");
        }
        let login = login_cookie("st", 600);
        for part in [
            "admin_login=st",
            "HttpOnly",
            "Secure",
            "SameSite=Lax",
            "Path=/",
            "Max-Age=600",
        ] {
            assert!(login.contains(part), "{login}");
        }
    }

    #[test]
    fn download_names_cannot_break_the_header() {
        assert_eq!(
            safe_download_name("Brazil Livery (v2).zip"),
            "Brazil Livery (v2).zip"
        );
        assert_eq!(safe_download_name("a\"b\r\nc;é.zip"), "a_b__c__.zip");
        assert_eq!(safe_download_name(""), "download");
    }

    #[test]
    fn sections_show_an_empty_state() {
        let html = section("reports", "Open reports", 0, "<b>x</b>");
        assert!(html.contains("Nothing here."));
        assert!(!html.contains("<b>x</b>"));
    }
}
