//! Mod hosting, read side: public catalog, mod pages, file downloads, "my mods"
//! and the admin review endpoints. The upload endpoints live in `uploads.rs`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path as UrlPath, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::SecondsFormat;
use community_application::ports::Hasher;
use community_application::{AppError, ModHostingService};
use community_domain::mods::{ModVersion, PublishedMod};
use community_infrastructure::Sha256Hasher;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use tower_http::services::ServeDir;

use crate::{ApiError, AppState, AuthenticatedUser};

/// Everything the mod hosting handlers need beyond the generic state.
#[derive(Clone)]
pub struct ModHosting {
    pub service: Arc<ModHostingService>,
    /// Public origin without a trailing slash (used to build download and page URLs).
    pub public_base_url: String,
    /// The `public/` area of the storage root: the only directory ever served.
    pub files_dir: PathBuf,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/catalog", get(catalog))
        .route("/mods/mine", get(mine))
        .route("/mods/{id}", get(mod_page))
        .route("/admin/mods/pending", get(pending))
        .route(
            "/admin/mods/{mod_id}/versions/{version}/approve",
            post(approve),
        )
        .route(
            "/admin/mods/{mod_id}/versions/{version}/reject",
            post(reject),
        )
}

/// `GET /files/*`: approved files only, with `Range` support. Pending and
/// incoming files live in sibling directories that are never served.
pub fn files_router(public_dir: &Path) -> Router<AppState> {
    Router::new().nest_service("/files", ServeDir::new(public_dir))
}

/// Percent-encodes one URL path segment (unreserved characters stay).
pub fn encode_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `{base}/files/<mod>/<version>/<file>` for a storage key.
pub fn file_url(base: &str, storage_key: &str) -> String {
    let path: Vec<String> = storage_key.split('/').map(encode_segment).collect();
    format!("{base}/files/{}", path.join("/"))
}

fn timestamp(time: chrono::DateTime<chrono::Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// The JSON shape of a version for its creator and for admins.
pub fn version_json(version: &ModVersion) -> Value {
    json!({
        "modId": version.mod_id.as_str(),
        "version": version.version,
        "title": version.title,
        "description": version.description,
        "longDescription": version.long_description,
        "category": version.category,
        "game": version.game,
        "fileName": version.file_name,
        "size": version.size,
        "sha256": version.sha256,
        "status": version.status.as_str(),
        "rejectionReason": version.rejection_reason,
        "screenshotCount": version.screenshots.len(),
        "uploadedAt": timestamp(version.uploaded_at),
        "reviewedAt": version.reviewed_at.map(timestamp),
    })
}

/// One entry of `GET /catalog`, in the shape of `catalog/catalog.json`.
pub fn catalog_entry(base: &str, published: &PublishedMod) -> Value {
    let version = &published.version;
    let updated = version.reviewed_at.unwrap_or(version.uploaded_at);
    let screenshots: Vec<String> = version
        .screenshots
        .iter()
        .map(|shot| file_url(base, shot.storage_key.as_str()))
        .collect();
    let mut entry = Map::new();
    entry.insert("id".into(), json!(version.mod_id.as_str()));
    entry.insert("title".into(), json!(version.title));
    entry.insert("author".into(), json!(published.author));
    entry.insert("game".into(), json!(version.game));
    entry.insert("category".into(), json!(version.category));
    entry.insert("description".into(), json!(version.description));
    entry.insert("version".into(), json!(version.version));
    entry.insert(
        "updatedAt".into(),
        json!(updated.format("%Y-%m-%d").to_string()),
    );
    entry.insert(
        "pageUrl".into(),
        json!(format!("{base}/mods/{}", version.mod_id.as_str())),
    );
    if let Some(first) = screenshots.first() {
        entry.insert("thumbnailUrl".into(), json!(first));
        entry.insert("screenshots".into(), json!(screenshots));
    }
    if let Some(long) = &version.long_description {
        entry.insert("longDescription".into(), json!(long));
    }
    entry.insert(
        "download".into(),
        json!({
            "url": file_url(base, version.storage_key.as_str()),
            "size": version.size,
            "sha256": version.sha256,
        }),
    );
    Value::Object(entry)
}

/// Whether `If-None-Match` matches the strong `etag` (handles lists, `*` and weak tags).
pub fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    let Some(value) = headers.get(IF_NONE_MATCH).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    value.split(',').map(str::trim).any(|candidate| {
        candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == etag
    })
}

/// `GET /catalog`: the latest approved version of every mod, with an `ETag`.
async fn catalog(State(app): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let published = app.mods.service.public_catalog().await?;
    let entries: Vec<Value> = published
        .iter()
        .map(|p| catalog_entry(&app.mods.public_base_url, p))
        .collect();
    let body = json!({ "schemaVersion": 1, "entries": entries }).to_string();
    let etag = format!("\"{}\"", Sha256Hasher.hash(&body));

    let mut response = if etag_matches(&headers, &etag) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        let mut ok = body.into_response();
        ok.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        ok
    };
    let tag = HeaderValue::from_str(&etag)
        .map_err(|_| AppError::Internal("etag is not a valid header".into()))?;
    response.headers_mut().insert(ETAG, tag);
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    Ok(response)
}

/// `GET /mods/mine`: the signed-in user's mods with every version and its status.
async fn mine(
    State(app): State<AppState>,
    auth: AuthenticatedUser,
) -> Result<Json<Value>, ApiError> {
    let mods = app.mods.service.my_mods(&auth.user).await?;
    let mods: Vec<Value> = mods
        .iter()
        .map(|m| {
            json!({
                "id": m.mod_id.as_str(),
                "versions": m.versions.iter().map(version_json).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(Json(json!({ "mods": mods })))
}

pub fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Minimal escaped HTML page of a mod (the catalog's `pageUrl`).
pub fn mod_page_html(published: &PublishedMod) -> String {
    let version = &published.version;
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{title}</title></head><body><h1>{title}</h1>\
         <p>by {author} &middot; version {version}</p><p>{description}</p></body></html>",
        title = escape_html(&version.title),
        author = escape_html(&published.author),
        version = escape_html(&version.version),
        description = escape_html(&version.description),
    )
}

/// `GET /mods/{id}`: the public page of an approved mod.
async fn mod_page(
    State(app): State<AppState>,
    UrlPath(id): UrlPath<String>,
) -> Result<Response, ApiError> {
    let published = app
        .mods
        .service
        .public_catalog()
        .await?
        .into_iter()
        .find(|p| p.version.mod_id.as_str() == id)
        .ok_or(AppError::NotFound)?;
    let mut response = mod_page_html(&published).into_response();
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'none'"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

/// `GET /admin/mods/pending`: versions waiting for review (admins only).
async fn pending(
    State(app): State<AppState>,
    auth: AuthenticatedUser,
) -> Result<Json<Value>, ApiError> {
    let versions = app.mods.service.list_pending(&auth.user).await?;
    Ok(Json(json!({
        "versions": versions.iter().map(version_json).collect::<Vec<_>>(),
    })))
}

/// `POST /admin/mods/{mod}/versions/{version}/approve`.
async fn approve(
    State(app): State<AppState>,
    auth: AuthenticatedUser,
    UrlPath((mod_id, version)): UrlPath<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    app.mods
        .service
        .approve(&auth.user, &mod_id, &version)
        .await?;
    Ok(Json(json!({ "status": "approved" })))
}

#[derive(Deserialize)]
struct RejectBody {
    reason: String,
}

/// `POST /admin/mods/{mod}/versions/{version}/reject` with `{"reason"}`.
async fn reject(
    State(app): State<AppState>,
    auth: AuthenticatedUser,
    UrlPath((mod_id, version)): UrlPath<(String, String)>,
    body: Result<Json<RejectBody>, JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    // The admin check comes first so non-admins learn nothing about the body rules.
    if !app.mods.service.is_admin(&auth.user) {
        return Err(AppError::Forbidden.into());
    }
    let Json(body) = body.map_err(|_| AppError::BadRequest("The JSON body is invalid.".into()))?;
    app.mods
        .service
        .reject(&auth.user, &mod_id, &version, &body.reason)
        .await?;
    Ok(Json(json!({ "status": "rejected" })))
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use community_domain::mods::{ModSlug, Screenshot, StorageKey, VersionStatus};
    use community_domain::UserId;

    use super::*;

    fn published(title: &str) -> PublishedMod {
        PublishedMod {
            author: "Rider <1>".into(),
            version: ModVersion {
                id: "v1".into(),
                mod_id: ModSlug::parse("brazil-livery").unwrap(),
                owner_user_id: UserId("u1".into()),
                version: "1.0".into(),
                title: title.into(),
                description: "A \"livery\" & more".into(),
                long_description: Some("Long".into()),
                category: "liveries".into(),
                game: "MotoGP 26".into(),
                file_name: "Brazil Livery.zip".into(),
                size: 10,
                sha256: "a".repeat(64),
                storage_key: StorageKey::from_stored("brazil-livery/1.0/Brazil Livery.zip".into()),
                status: VersionStatus::Approved,
                rejection_reason: None,
                uploaded_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
                reviewed_at: DateTime::from_timestamp(1_700_086_400, 0),
                screenshots: vec![Screenshot {
                    index: 0,
                    storage_key: StorageKey::from_stored(
                        "brazil-livery/1.0/screenshot-0.png".into(),
                    ),
                    content_type: "image/png".into(),
                }],
            },
        }
    }

    #[test]
    fn segments_are_percent_encoded() {
        assert_eq!(
            encode_segment("Brazil Livery (v2).zip"),
            "Brazil%20Livery%20%28v2%29.zip"
        );
        assert_eq!(encode_segment("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(
            file_url("https://x.test", "m/1.0/a b.zip"),
            "https://x.test/files/m/1.0/a%20b.zip"
        );
    }

    #[test]
    fn catalog_entries_follow_the_catalog_shape() {
        let entry = catalog_entry("https://x.test", &published("Brazil Livery"));
        assert_eq!(entry["id"], "brazil-livery");
        assert_eq!(entry["author"], "Rider <1>");
        assert_eq!(entry["updatedAt"], "2023-11-15");
        assert_eq!(entry["pageUrl"], "https://x.test/mods/brazil-livery");
        assert_eq!(entry["longDescription"], "Long");
        assert_eq!(
            entry["thumbnailUrl"],
            "https://x.test/files/brazil-livery/1.0/screenshot-0.png"
        );
        assert_eq!(
            entry["download"]["url"],
            "https://x.test/files/brazil-livery/1.0/Brazil%20Livery.zip"
        );
        assert_eq!(entry["download"]["size"], 10);
    }

    #[test]
    fn entries_without_screenshots_or_long_text_omit_those_keys() {
        let mut item = published("T");
        item.version.screenshots.clear();
        item.version.long_description = None;
        let entry = catalog_entry("https://x.test", &item);
        assert!(entry.get("screenshots").is_none());
        assert!(entry.get("thumbnailUrl").is_none());
        assert!(entry.get("longDescription").is_none());
    }

    #[test]
    fn version_json_is_camel_case() {
        let value = version_json(&published("T").version);
        assert_eq!(value["fileName"], "Brazil Livery.zip");
        assert_eq!(value["status"], "approved");
        assert_eq!(value["screenshotCount"], 1);
        assert!(value["rejectionReason"].is_null());
    }

    #[test]
    fn the_mod_page_escapes_everything() {
        let html = mod_page_html(&published("<script>alert(1)</script>"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("Rider &lt;1&gt;"));
        assert!(html.contains("A &quot;livery&quot; &amp; more"));
        assert_eq!(escape_html("it's"), "it&#39;s");
    }

    #[test]
    fn etags_match_lists_wildcards_and_weak_tags() {
        let with = |value: &str| {
            let mut headers = HeaderMap::new();
            headers.insert(IF_NONE_MATCH, HeaderValue::from_str(value).unwrap());
            headers
        };
        assert!(etag_matches(&with("\"abc\""), "\"abc\""));
        assert!(etag_matches(&with("\"x\", \"abc\""), "\"abc\""));
        assert!(etag_matches(&with("W/\"abc\""), "\"abc\""));
        assert!(etag_matches(&with("*"), "\"abc\""));
        assert!(!etag_matches(&with("\"other\""), "\"abc\""));
        assert!(!etag_matches(&HeaderMap::new(), "\"abc\""));
    }
}
