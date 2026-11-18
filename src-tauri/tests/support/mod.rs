//! Shared support for the interface integration tests: `cargo metadata`
//! reading and the Clean Architecture dependency-rule validator.
#![allow(dead_code)]

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Cargo workspace root (one level above `src-tauri`).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri must live inside the workspace root")
        .to_path_buf()
}

/// Runs `cargo metadata --no-deps` for the given manifest.
pub fn cargo_metadata(manifest: &Path) -> Value {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let output = Command::new(cargo)
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(manifest)
        .output()
        .expect("failed to run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("cargo metadata output is not JSON")
}

/// Sorted names of the workspace member packages described by the metadata.
pub fn member_names(metadata: &Value) -> Vec<String> {
    let mut names: Vec<String> = metadata["packages"]
        .as_array()
        .expect("metadata without packages")
        .iter()
        .map(|p| p["name"].as_str().unwrap_or_default().to_string())
        .collect();
    names.sort();
    names
}

/// Allowed runtime dependencies per package (`None` = unrestricted).
/// Dev-dependencies are never restricted: that is where test tooling lives.
fn allowed_runtime_deps(package: &str) -> Option<&'static [&'static str]> {
    match package {
        "motogp-domain" => Some(&[]),
        "motogp-application" => Some(&["motogp-domain"]),
        _ => None, // adapters may use external libs; the interface composes everything
    }
}

/// Inner layers that may only depend inward among themselves.
const LAYERS: [&str; 3] = ["motogp-domain", "motogp-application", "motogp-infrastructure"];

/// Returns the violations of `interface -> infrastructure -> application -> domain`.
pub fn dependency_violations(metadata: &Value) -> Vec<String> {
    let mut violations = Vec::new();
    for package in metadata["packages"]
        .as_array()
        .expect("metadata without packages")
    {
        let name = package["name"].as_str().unwrap_or_default();
        for dep in runtime_deps(package) {
            match allowed_runtime_deps(name) {
                Some(allowed) if !allowed.contains(&dep.as_str()) => {
                    let kind = if LAYERS.contains(&dep.as_str()) {
                        "outward"
                    } else {
                        "external"
                    };
                    violations.push(format!(
                        "{name} has a disallowed {kind} production dependency: {dep}"
                    ));
                }
                None if dep == "motogp-launcher" => {
                    violations.push(format!("{name} depends on the interface ({dep})"));
                }
                _ => {}
            }
        }
    }
    violations
}

/// Normal and build dependencies (everything that is not `dev`).
fn runtime_deps(package: &Value) -> Vec<String> {
    package["dependencies"]
        .as_array()
        .map(|deps| {
            deps.iter()
                .filter(|d| d["kind"].as_str() != Some("dev"))
                .filter_map(|d| d["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}
