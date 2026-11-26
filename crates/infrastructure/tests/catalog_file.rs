//! The catalog shipped in the repository (issue #72): it must pass the strict
//! validation, and `catalog.schema.json` must stay consistent with the Rust rules.
use motogp_application::validate_catalog_strict;
use motogp_infrastructure::parse_catalog;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn catalog_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalog")
}

fn schema() -> Value {
    let text = fs::read_to_string(catalog_dir().join("catalog.schema.json")).expect("schema file");
    serde_json::from_str(&text).expect("schema is JSON")
}

fn strict(body: &str) -> Result<usize, Vec<String>> {
    let raw = parse_catalog(body.as_bytes()).map_err(|e| vec![e.to_string()])?;
    validate_catalog_strict(raw).map(|entries| entries.len())
}

#[test]
fn the_repository_catalog_is_valid() {
    let bytes = fs::read(catalog_dir().join("catalog.json")).expect("catalog file");
    let raw = parse_catalog(&bytes).expect("catalog parses");
    let result = validate_catalog_strict(raw);
    assert!(result.is_ok(), "catalog problems: {:?}", result.err());
}

#[test]
fn schema_requires_the_fields_the_rust_rules_require() {
    let schema = schema();
    let entry = &schema["$defs"]["entry"];
    let required: Vec<&str> = entry["required"]
        .as_array()
        .expect("entry.required")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for field in [
        "id",
        "title",
        "author",
        "game",
        "category",
        "description",
        "version",
        "updatedAt",
        "pageUrl",
    ] {
        assert!(required.contains(&field), "{field} must be required");
    }
    for optional in ["thumbnailUrl", "screenshots", "longDescription", "download"] {
        assert!(!required.contains(&optional), "{optional} is optional");
        assert!(entry["properties"].get(optional).is_some());
    }
    assert_eq!(entry["additionalProperties"], Value::Bool(false));
    assert_eq!(schema["additionalProperties"], Value::Bool(false));
    assert_eq!(schema["properties"]["schemaVersion"]["const"], 1);
}

#[test]
fn schema_url_and_hash_patterns_match_the_rust_rules() {
    let schema = schema();
    let defs = &schema["$defs"];
    let https = defs["httpsUrl"]["pattern"].as_str().expect("https pattern");
    assert!(https.starts_with("^https://"), "{https}");
    // pageUrl, thumbnailUrl and download.url all go through the https definition.
    let entry = &defs["entry"]["properties"];
    assert_eq!(entry["pageUrl"]["$ref"], "#/$defs/httpsUrl");
    assert_eq!(entry["thumbnailUrl"]["$ref"], "#/$defs/httpsUrl");
    let shots = &entry["screenshots"];
    assert_eq!(shots["items"]["$ref"], "#/$defs/httpsUrl");
    assert_eq!(
        (shots["minItems"].as_u64(), shots["maxItems"].as_u64()),
        (Some(1), Some(8))
    );
    assert_eq!(shots["uniqueItems"], Value::Bool(true));
    assert_eq!(entry["longDescription"]["maxLength"], 20000);
    assert_eq!(
        defs["download"]["properties"]["url"]["$ref"],
        "#/$defs/httpsUrl"
    );

    let download = &defs["download"];
    assert_eq!(
        download["properties"]["sha256"]["pattern"],
        "^[0-9a-f]{64}$"
    );
    assert_eq!(download["properties"]["size"]["minimum"], 1);
    assert_eq!(download["additionalProperties"], Value::Bool(false));
    let required: Vec<&str> = download["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for field in ["url", "size", "sha256"] {
        assert!(
            required.contains(&field),
            "download.{field} must be required"
        );
    }

    let id = &entry["id"];
    assert_eq!(id["pattern"], "^[a-z0-9]+(-[a-z0-9]+)*$");
    assert_eq!(id["maxLength"], 64);
}

#[test]
fn strict_validation_accepts_a_good_catalog() {
    let good = r#"{"schemaVersion":1,"entries":[{"id":"a-mod","title":"T","author":"A","game":"G","category":"C","description":"D","version":"1","updatedAt":"2026-01-02","pageUrl":"https://example.com/a"}]}"#;
    assert_eq!(strict(good), Ok(1));
}

#[test]
fn strict_validation_rejects_an_insecure_url() {
    let bad = r#"{"schemaVersion":1,"entries":[{"id":"a-mod","title":"T","author":"A","game":"G","category":"C","description":"D","version":"1","updatedAt":"2026-01-02","pageUrl":"http://example.com/a"}]}"#;
    assert!(strict(bad).is_err());
}

#[test]
fn strict_validation_rejects_duplicate_ids() {
    let entry = r#"{"id":"a-mod","title":"T","author":"A","game":"G","category":"C","description":"D","version":"1","updatedAt":"2026-01-02","pageUrl":"https://example.com/a"}"#;
    let bad = format!(r#"{{"schemaVersion":1,"entries":[{entry},{entry}]}}"#);
    assert!(strict(&bad).is_err());
}
