//! The sponsor feed shipped in the repository (issue #106): `catalog/ads.json` must pass
//! the rules below and `catalog/ads.schema.json` must stay consistent with them.
//! Self-contained on purpose (serde_json + std only): the Rust domain types for
//! creatives live in another issue.
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_IMAGE_BYTES: u64 = 200 * 1024;
const SLOTS: [&str; 3] = ["discover", "sidebar", "about"];
const FEED_FIELDS: [&str; 2] = ["version", "creatives"];
const CREATIVE_FIELDS: [&str; 10] = [
    "id",
    "slot",
    "title",
    "body",
    "image",
    "click_url",
    "sponsor_label",
    "impression_url",
    "starts_at",
    "expires_at",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

// ---------- the check ----------

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_secs() as i64;
    let (y, m, d) = civil_from_days(secs / 86_400);
    format!("{y:04}-{m:02}-{d:02}")
}

fn is_valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| s[r].bytes().all(|c| c.is_ascii_digit());
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let (y, m, d): (i64, i64, i64) = (
        s[0..4].parse().unwrap(),
        s[5..7].parse().unwrap(),
        s[8..10].parse().unwrap(),
    );
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let max = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => return false,
    };
    d >= 1 && d <= max
}

fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// `https://` only, with a real public host name.
fn check_https_url(value: &str) -> Result<(), String> {
    let rest = value
        .strip_prefix("https://")
        .ok_or("must start with https://")?;
    if value.chars().any(char::is_whitespace) {
        return Err("must not contain whitespace".into());
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return Err("must not contain user info".into());
    }
    if authority.contains(':') || authority.contains('[') {
        return Err("must not contain an explicit port or IP literal".into());
    }
    if authority.eq_ignore_ascii_case("localhost") {
        return Err("host must not be localhost".into());
    }
    if authority.bytes().all(|c| c.is_ascii_digit() || c == b'.') {
        return Err("host must not be an IP literal".into());
    }
    if !authority.contains('.') || authority.starts_with('.') || authority.ends_with('.') {
        return Err("host must be a public name with a dot".into());
    }
    Ok(())
}

fn string_field<'a>(c: &'a Value, field: &str) -> Option<&'a str> {
    c.get(field).and_then(Value::as_str)
}

fn check_text(c: &Value, id: &str, field: &str, max: usize) -> Vec<String> {
    match string_field(c, field) {
        None => vec![format!("{id}: {field} is required")],
        Some(s) if s.trim().is_empty() => vec![format!("{id}: {field} must not be blank")],
        Some(s) if s.chars().count() > max => {
            vec![format!("{id}: {field} must be at most {max} characters")]
        }
        Some(_) => vec![],
    }
}

fn check_id(c: &Value, seen: &mut HashSet<String>) -> Vec<String> {
    match string_field(c, "id") {
        None => vec!["<unknown>: id is required".into()],
        Some(id) => {
            let mut problems = vec![];
            if !is_slug(id) || id.len() > 64 {
                problems.push(format!("{id}: id must be a lowercase slug of at most 64"));
            }
            if !seen.insert(id.to_string()) {
                problems.push(format!("{id}: duplicate id"));
            }
            problems
        }
    }
}

fn check_slot(c: &Value, id: &str) -> Vec<String> {
    match string_field(c, "slot") {
        Some(s) if SLOTS.contains(&s) => vec![],
        _ => vec![format!("{id}: slot must be one of {SLOTS:?}")],
    }
}

fn check_url_field(c: &Value, id: &str, field: &str, required: bool) -> Vec<String> {
    match c.get(field) {
        None if required => vec![format!("{id}: {field} is required")],
        None => vec![],
        Some(v) => match v.as_str() {
            Some(s) => check_https_url(s)
                .err()
                .map(|e| format!("{id}: {field} {e}"))
                .into_iter()
                .collect(),
            None => vec![format!("{id}: {field} must be a string")],
        },
    }
}

fn check_image(c: &Value, id: &str, base: &Path) -> Vec<String> {
    let Some(image) = string_field(c, "image") else {
        return vec![format!("{id}: image is required")];
    };
    let ext_ok = image
        .rsplit('.')
        .next()
        .map(|e| ["png", "jpg", "jpeg", "webp"].contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
        && image.contains('.');
    let mut problems = vec![];
    if !ext_ok {
        problems.push(format!("{id}: image must be .png, .jpg, .jpeg or .webp"));
    }
    if image.starts_with("https://") {
        if let Err(e) = check_https_url(image) {
            problems.push(format!("{id}: image {e}"));
        }
        return problems;
    }
    if image.is_empty()
        || image.starts_with('/')
        || image.contains('\\')
        || image.split('/').any(|p| p == ".." || p.is_empty())
        || image.contains(':')
    {
        problems.push(format!(
            "{id}: image must be a repo-relative path without leading '/', '..' or '\\'"
        ));
        return problems;
    }
    match fs::metadata(base.join(image)) {
        Ok(m) if m.is_file() => {
            if m.len() > MAX_IMAGE_BYTES {
                problems.push(format!(
                    "{id}: image is {} bytes, the limit is {MAX_IMAGE_BYTES}",
                    m.len()
                ));
            }
        }
        _ => problems.push(format!("{id}: image file {image} does not exist")),
    }
    problems
}

fn check_dates(c: &Value, id: &str) -> Vec<String> {
    let mut problems = vec![];
    let mut dates: [Option<&str>; 2] = [None, None];
    for (i, field) in ["starts_at", "expires_at"].into_iter().enumerate() {
        if let Some(v) = c.get(field) {
            match v.as_str().filter(|s| is_valid_date(s)) {
                Some(s) => dates[i] = Some(s),
                None => problems.push(format!("{id}: {field} must be YYYY-MM-DD")),
            }
        }
    }
    if let (Some(start), Some(end)) = (dates[0], dates[1]) {
        if start > end {
            problems.push(format!("{id}: starts_at must not be after expires_at"));
        }
    }
    // Expiry is deliberately not a problem: it would turn CI red for unrelated PRs
    // once a sponsor ends. See `expiry_warnings`.
    problems
}

/// Expired creatives are housekeeping, not a failure: the launcher hides them at runtime.
fn expiry_warnings(text: &str, today: &str) -> Vec<String> {
    let Ok(feed) = serde_json::from_str::<Value>(text) else {
        return vec![];
    };
    feed.get("creatives")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| {
            let end = string_field(c, "expires_at").filter(|s| is_valid_date(s))?;
            (end < today).then(|| {
                let id = string_field(c, "id").unwrap_or("<unknown>");
                format!("{id}: expired on {end}, remove it by PR")
            })
        })
        .collect()
}

fn check_unknown_fields(value: &Value, allowed: &[&str], scope: &str) -> Vec<String> {
    value
        .as_object()
        .map(|o| {
            o.keys()
                .filter(|k| !allowed.contains(&k.as_str()))
                .map(|k| format!("{scope}: unknown field {k}"))
                .collect()
        })
        .unwrap_or_default()
}

/// Runs every rule and reports every problem, not only the first.
fn check_feed(text: &str, base: &Path) -> Vec<String> {
    let feed: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => return vec![format!("feed is not valid JSON: {e}")],
    };
    let mut problems = check_unknown_fields(&feed, &FEED_FIELDS, "feed");
    if feed.get("version") != Some(&Value::from(1)) {
        problems.push("feed: version must be 1".into());
    }
    let Some(creatives) = feed.get("creatives").and_then(Value::as_array) else {
        problems.push("feed: creatives must be an array".into());
        return problems;
    };
    let mut seen = HashSet::new();
    for c in creatives {
        let id = string_field(c, "id").unwrap_or("<unknown>").to_string();
        problems.extend(check_unknown_fields(c, &CREATIVE_FIELDS, &id));
        problems.extend(check_id(c, &mut seen));
        problems.extend(check_slot(c, &id));
        problems.extend(check_text(c, &id, "title", 60));
        problems.extend(check_text(c, &id, "body", 140));
        problems.extend(check_text(c, &id, "sponsor_label", 40));
        problems.extend(check_url_field(c, &id, "click_url", true));
        problems.extend(check_url_field(c, &id, "impression_url", false));
        problems.extend(check_image(c, &id, base));
        problems.extend(check_dates(c, &id));
    }
    problems
}

// ---------- helpers for the tests ----------

fn good() -> Value {
    serde_json::json!({
        "id": "acme",
        "slot": "sidebar",
        "title": "Acme",
        "body": "Racing parts",
        "image": "catalog/ads/house-support.png",
        "click_url": "https://example.com/acme",
        "sponsor_label": "Sponsored"
    })
}

fn feed_of(creatives: Vec<Value>) -> String {
    serde_json::json!({ "version": 1, "creatives": creatives }).to_string()
}

/// A feed with one good creative where `field` is replaced (or removed when `None`).
fn problems_with(field: &str, value: Option<Value>) -> Vec<String> {
    let mut c = good();
    match value {
        Some(v) => c[field] = v,
        None => {
            c.as_object_mut().unwrap().remove(field);
        }
    }
    check_feed(&feed_of(vec![c]), &repo_root())
}

fn assert_names(problems: &[String], field: &str) {
    assert!(
        problems.iter().any(|p| p.contains(field)),
        "expected a problem naming {field}, got {problems:?}"
    );
}

// ---------- scenarios ----------

#[test]
fn the_shipped_feed_is_valid() {
    let text = fs::read_to_string(repo_root().join("catalog/ads.json")).expect("ads.json");
    let problems = check_feed(&text, &repo_root());
    assert!(problems.is_empty(), "feed problems: {problems:?}");
    for warning in expiry_warnings(&text, &today()) {
        eprintln!("warning: {warning}");
    }
}

#[test]
fn the_shipped_feed_has_a_house_ad_in_the_about_slot_pointing_at_the_support_page() {
    let text = fs::read_to_string(repo_root().join("catalog/ads.json")).expect("ads.json");
    let feed: Value = serde_json::from_str(&text).unwrap();
    let found = feed["creatives"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["slot"] == "about" && c["click_url"] == motogp_domain::SUPPORT_PAGE_URL);
    assert!(found, "no about-slot house ad with the support page URL");
}

#[test]
fn a_good_creative_passes() {
    assert!(problems_with("title", Some("Acme".into())).is_empty());
}

#[test]
fn click_url_must_be_https() {
    assert_names(
        &problems_with("click_url", Some("http://example.com/".into())),
        "click_url",
    );
}

#[test]
fn click_url_must_not_have_user_info() {
    assert_names(
        &problems_with("click_url", Some("https://user@example.com/".into())),
        "click_url",
    );
}

#[test]
fn click_url_must_not_be_an_ip_literal() {
    assert_names(
        &problems_with("click_url", Some("https://127.0.0.1/".into())),
        "click_url",
    );
}

#[test]
fn click_url_must_not_be_localhost() {
    assert_names(
        &problems_with("click_url", Some("https://localhost/".into())),
        "click_url",
    );
}

#[test]
fn click_url_must_not_have_a_port() {
    assert_names(
        &problems_with("click_url", Some("https://example.com:8443/".into())),
        "click_url",
    );
}

#[test]
fn sponsor_label_is_required() {
    assert_names(&problems_with("sponsor_label", None), "sponsor_label");
}

#[test]
fn sponsor_label_must_not_be_blank() {
    assert_names(
        &problems_with("sponsor_label", Some("   ".into())),
        "sponsor_label",
    );
}

#[test]
fn sponsor_label_has_a_length_cap() {
    assert_names(
        &problems_with("sponsor_label", Some("x".repeat(41).into())),
        "sponsor_label",
    );
}

#[test]
fn title_and_body_have_length_caps() {
    assert_names(
        &problems_with("title", Some("x".repeat(61).into())),
        "title",
    );
    assert_names(&problems_with("body", Some("x".repeat(141).into())), "body");
}

#[test]
fn slot_must_be_known() {
    assert_names(&problems_with("slot", Some("header".into())), "slot");
}

#[test]
fn image_must_not_be_an_svg() {
    assert_names(
        &problems_with("image", Some("catalog/ads/logo.svg".into())),
        "image",
    );
}

#[test]
fn image_must_not_escape_the_repository() {
    assert_names(
        &problems_with("image", Some("../outside.png".into())),
        "image",
    );
}

#[test]
fn image_file_must_exist() {
    assert_names(
        &problems_with("image", Some("catalog/ads/missing.png".into())),
        "image",
    );
}

#[test]
fn image_may_be_an_https_url() {
    assert!(problems_with("image", Some("https://cdn.example.com/a.png".into())).is_empty());
    assert_names(
        &problems_with("image", Some("https://localhost/a.png".into())),
        "image",
    );
}

#[test]
fn image_file_over_200_kib_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("ads")).unwrap();
    fs::write(
        dir.path().join("ads/big.png"),
        vec![0u8; MAX_IMAGE_BYTES as usize + 1],
    )
    .unwrap();
    fs::write(
        dir.path().join("ads/ok.png"),
        vec![0u8; MAX_IMAGE_BYTES as usize],
    )
    .unwrap();
    let mut c = good();
    c["image"] = "ads/big.png".into();
    assert_names(&check_feed(&feed_of(vec![c.clone()]), dir.path()), "image");
    c["image"] = "ads/ok.png".into();
    assert!(check_feed(&feed_of(vec![c]), dir.path()).is_empty());
}

#[test]
fn impression_url_must_be_https() {
    assert_names(
        &problems_with("impression_url", Some("http://track.example.com/".into())),
        "impression_url",
    );
}

#[test]
fn an_expired_creative_is_a_warning_not_a_failure() {
    let mut c = good();
    c["expires_at"] = "2026-05-31".into();
    let feed = feed_of(vec![c]);
    assert!(check_feed(&feed, &repo_root()).is_empty());
    let warnings = expiry_warnings(&feed, "2026-06-01");
    assert_eq!(warnings.len(), 1, "got {warnings:?}");
    assert!(warnings[0].contains("acme") && warnings[0].contains("2026-05-31"));

    let mut c = good();
    c["expires_at"] = "2026-06-01".into();
    assert!(expiry_warnings(&feed_of(vec![c]), "2026-06-01").is_empty());
}

#[test]
fn dates_must_be_valid_and_ordered() {
    assert_names(
        &problems_with("starts_at", Some("2026-13-01".into())),
        "starts_at",
    );
    let mut c = good();
    c["starts_at"] = "2026-09-01".into();
    c["expires_at"] = "2026-08-01".into();
    let problems = check_feed(&feed_of(vec![c]), &repo_root());
    assert_names(&problems, "starts_at must not be after");
}

#[test]
fn duplicate_ids_are_reported() {
    let problems = check_feed(&feed_of(vec![good(), good()]), &repo_root());
    assert_names(&problems, "duplicate id");
}

#[test]
fn id_must_be_a_slug() {
    assert_names(&problems_with("id", Some("Not A Slug".into())), "id");
}

#[test]
fn unknown_fields_and_wrong_version_are_rejected() {
    assert_names(&problems_with("extra", Some("x".into())), "unknown field");
    let bad = r#"{"version":2,"creatives":[],"x":1}"#;
    let problems = check_feed(bad, &repo_root());
    assert_names(&problems, "version");
    assert_names(&problems, "unknown field x");
}

#[test]
fn every_problem_is_reported() {
    let mut c = good();
    c["slot"] = "header".into();
    c["click_url"] = "http://example.com/".into();
    c.as_object_mut().unwrap().remove("sponsor_label");
    let problems = check_feed(&feed_of(vec![c]), &repo_root());
    assert_eq!(problems.len(), 3, "{problems:?}");
}

#[test]
fn the_date_helper_matches_known_days() {
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    assert!(is_valid_date("2024-02-29"));
    assert!(!is_valid_date("2023-02-29"));
    assert!(!is_valid_date("2024-2-29"));
}

#[test]
fn the_schema_matches_the_check_rules() {
    let text = fs::read_to_string(repo_root().join("catalog/ads.schema.json")).expect("schema");
    let schema: Value = serde_json::from_str(&text).expect("schema is JSON");
    let creative = &schema["$defs"]["creative"];
    let required: Vec<&str> = creative["required"]
        .as_array()
        .expect("creative.required")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for field in [
        "id",
        "slot",
        "title",
        "body",
        "image",
        "click_url",
        "sponsor_label",
    ] {
        assert!(required.contains(&field), "{field} must be required");
    }
    for optional in ["starts_at", "expires_at", "impression_url"] {
        assert!(!required.contains(&optional), "{optional} is optional");
        assert!(creative["properties"].get(optional).is_some());
    }
    let slots: Vec<&str> = creative["properties"]["slot"]["enum"]
        .as_array()
        .expect("slot enum")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(slots, SLOTS);
    assert_eq!(creative["additionalProperties"], Value::Bool(false));
    assert_eq!(schema["additionalProperties"], Value::Bool(false));
    assert_eq!(schema["properties"]["version"]["const"], 1);
    assert_eq!(creative["properties"]["id"]["maxLength"], 64);
    assert_eq!(creative["properties"]["title"]["maxLength"], 60);
    assert_eq!(creative["properties"]["body"]["maxLength"], 140);
    assert_eq!(creative["properties"]["sponsor_label"]["maxLength"], 40);
}

/// Collects every "pattern" string in a JSON value.
fn collect_patterns(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(m) => {
            for (k, child) in m {
                if k == "pattern" {
                    if let Some(s) = child.as_str() {
                        out.push(s.to_string());
                    }
                }
                collect_patterns(child, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|c| collect_patterns(c, out)),
        _ => {}
    }
}

#[test]
fn schema_patterns_are_ecma_262_without_inline_flags() {
    // JSON Schema patterns are ECMA-262 regexes: inline flag groups such as `(?i)`
    // are invalid there, so validators (ajv, VS Code) could not compile the schema.
    for name in ["ads.schema.json", "catalog.schema.json"] {
        let text = fs::read_to_string(repo_root().join("catalog").join(name)).expect("schema");
        let schema: Value = serde_json::from_str(&text).expect("schema json");
        let mut patterns = Vec::new();
        collect_patterns(&schema, &mut patterns);
        assert!(!patterns.is_empty(), "{name}: no patterns found");
        for p in patterns {
            assert!(
                !p.contains("(?"),
                "{name}: inline flag syntax in pattern {p}"
            );
        }
    }
}
