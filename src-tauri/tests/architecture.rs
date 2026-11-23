//! Gate 6 (Cargo architecture) and workspace integrity.
mod support;

use std::path::PathBuf;
use support::{cargo_metadata, dependency_violations, member_names, workspace_root};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .join("Cargo.toml")
}

#[test]
fn workspace_members_are_registered() {
    let metadata = cargo_metadata(&workspace_root().join("Cargo.toml"));
    assert_eq!(
        member_names(&metadata),
        vec![
            "motogp-application",
            "motogp-domain",
            "motogp-infrastructure",
            "motogp-launcher",
        ]
    );
}

#[test]
fn rejects_outward_cargo_dependencies() {
    let metadata = cargo_metadata(&fixture("outward-deps"));
    let violations = dependency_violations(&metadata);
    assert!(
        violations
            .iter()
            .any(|v| v.contains("motogp-application") && v.contains("motogp-infrastructure")),
        "application -> infrastructure should be rejected: {violations:?}"
    );
    assert!(
        violations
            .iter()
            .any(|v| v.contains("motogp-domain") && v.contains("serde")),
        "domain -> external dependency should be rejected: {violations:?}"
    );
}

#[test]
fn accepts_dev_dependencies_in_the_core() {
    let metadata = cargo_metadata(&fixture("dev-deps-ok"));
    assert_eq!(dependency_violations(&metadata), Vec::<String>::new());
}

#[test]
fn real_workspace_dependencies_point_inward() {
    let metadata = cargo_metadata(&workspace_root().join("Cargo.toml"));
    assert_eq!(dependency_violations(&metadata), Vec::<String>::new());
}

#[test]
fn updater_plugins_are_only_in_the_interface_crate() {
    let metadata = cargo_metadata(&workspace_root().join("Cargo.toml"));
    let runtime_deps = |package: &str| -> Vec<String> {
        let package = metadata["packages"]
            .as_array()
            .expect("metadata without packages")
            .iter()
            .find(|p| p["name"] == package)
            .unwrap_or_else(|| panic!("package {package} missing"));
        package["dependencies"]
            .as_array()
            .expect("dependencies")
            .iter()
            .filter(|d| d["kind"].is_null())
            .map(|d| d["name"].as_str().unwrap_or_default().to_string())
            .collect()
    };
    let plugins = ["tauri-plugin-updater", "tauri-plugin-process"];
    for inner in [
        "motogp-domain",
        "motogp-application",
        "motogp-infrastructure",
    ] {
        let deps = runtime_deps(inner);
        for plugin in plugins {
            assert!(
                !deps.iter().any(|d| d == plugin),
                "{inner} must not depend on {plugin}: the updater belongs to the interface layer"
            );
        }
    }
    let interface = runtime_deps("motogp-launcher");
    for plugin in plugins {
        assert!(
            interface.iter().any(|d| d == plugin),
            "motogp-launcher must depend on {plugin}"
        );
    }
}
