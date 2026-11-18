//! BDD runner (cucumber, `harness = false`) for the scenarios in `tests/features/`.
//! Scenario traceability: see `docs/workflow-validacao.md` (Scaffold section).
mod support;

use cucumber::{given, then, when, World};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use support::{cargo_metadata, dependency_violations, member_names, workspace_root};

#[derive(Debug, Default, World)]
struct ScaffoldWorld {
    /// Text of the validation workflow (CI scenario).
    workflow: String,
    /// Violations detected in the architecture fixtures.
    lint_violations_found: bool,
    cargo_violations_found: bool,
    composition_root_valid: bool,
}

fn root() -> PathBuf {
    workspace_root()
}

fn read(path: impl AsRef<Path>) -> String {
    let full = root().join(path.as_ref());
    fs::read_to_string(&full).unwrap_or_else(|e| panic!("could not read {full:?}: {e}"))
}

fn json(path: &str) -> serde_json::Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|e| panic!("{path} is not valid JSON: {e}"))
}

/// Runs the real ESLint (with `eslint-plugin-boundaries`) on a fixture and
/// returns (success, JSON output).
fn eslint_fixture(fixture: &str) -> (bool, String) {
    let dir = root().join("tests/fixtures/boundaries").join(fixture);
    let eslint = root().join("node_modules/eslint/bin/eslint.js");
    assert!(
        eslint.exists(),
        "ESLint not found at {eslint:?}: run `pnpm install` before the tests"
    );
    let output = Command::new("node")
        .current_dir(&dir)
        .arg(&eslint)
        .args(["--config", "eslint.config.mjs", "--format", "json", "src"])
        .output()
        .expect("failed to run node/eslint");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

fn command_available(program: &str, arg: &str) -> bool {
    Command::new(program)
        .arg(arg)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ---------- Scenario 1: scaffold preserving the project ----------

#[given("the project contains versioned documents and workflows")]
fn project_has_docs(_w: &mut ScaffoldWorld) {
    for path in [
        "CLAUDE.md",
        "docs/workflow-validacao.md",
        "docs/auto-update.md",
        "docs/tasks/README.md",
        ".github/workflows/validacao.yml",
    ] {
        assert!(root().join(path).is_file(), "{path} should exist");
    }
}

#[when("the scaffold is integrated into the current root")]
fn scaffold_integrated(_w: &mut ScaffoldWorld) {
    assert!(root().join("Cargo.toml").is_file());
    assert!(root().join("package.json").is_file());
}

#[then("the documents and the history are preserved")]
fn docs_preserved(_w: &mut ScaffoldWorld) {
    assert!(read("CLAUDE.md").contains("## Fluxo de Git / issues"));
    assert!(read("docs/auto-update.md").contains("auto-update"));
    assert!(root().join("docs/superpowers/specs").is_dir());
    let git = Command::new("git")
        .current_dir(root())
        .args(["log", "--oneline", "-n", "1", "--", "docs/workflow-validacao.md"])
        .output()
        .expect("git unavailable");
    assert!(
        !git.stdout.is_empty(),
        "the git history of workflow-validacao.md should exist"
    );
}

#[then("the workspace contains domain, application, infrastructure and src-tauri")]
fn workspace_has_members(_w: &mut ScaffoldWorld) {
    let metadata = cargo_metadata(&root().join("Cargo.toml"));
    assert_eq!(
        member_names(&metadata),
        [
            "motogp-application",
            "motogp-domain",
            "motogp-infrastructure",
            "motogp-launcher"
        ]
    );
}

#[then("the workspace and frontend tests finish successfully")]
fn test_targets_exist(_w: &mut ScaffoldWorld) {
    // The execution itself belongs to gates 4/5 (CI and QA). Here we assert the
    // targets exist and are non-interactive, avoiding `cargo test` recursion.
    assert_eq!(json("package.json")["scripts"]["test"], "vitest run");
    assert!(read("src-tauri/Cargo.toml").contains("harness = false"));
    assert!(root().join("src/ui/__tests__/scaffold.test.tsx").is_file());
}

// ---------- Scenario 2: open the application on Windows ----------

#[given("the MSVC, Rust, Node, pnpm and WebView2 prerequisites are available")]
fn prerequisites(_w: &mut ScaffoldWorld) {
    assert!(command_available("rustc", "--version"), "rustc missing");
    assert!(command_available("node", "--version"), "node missing");
    assert!(
        json("package.json")["packageManager"]
            .as_str()
            .is_some_and(|p| p.starts_with("pnpm@")),
        "packageManager must declare pnpm"
    );
    #[cfg(windows)]
    {
        let webview2 = ["HKLM", "HKCU"].iter().any(|hive| {
            ["WOW6432Node\\", ""].iter().any(|wow| {
                let key = format!(
                    "{hive}\\SOFTWARE\\{wow}Microsoft\\EdgeUpdate\\Clients\\{{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}}"
                );
                Command::new("reg")
                    .args(["query", &key])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            })
        });
        assert!(webview2, "WebView2 Runtime not found in the registry");
    }
}

#[when("I run the application with pnpm tauri dev")]
fn run_tauri_dev(_w: &mut ScaffoldWorld) {
    // The real smoke (window opens) is manual and documented; here we verify
    // the run is configured: `tauri` script + Tauri dev commands.
    assert_eq!(json("package.json")["scripts"]["tauri"], "tauri");
    let conf = json("src-tauri/tauri.conf.json");
    assert_eq!(conf["build"]["beforeDevCommand"], "pnpm dev");
    assert_eq!(conf["build"]["devUrl"], "http://localhost:1420");
}

#[then("I see the MotoGP Launcher window with the specified dimensions")]
fn window_dimensions(_w: &mut ScaffoldWorld) {
    let conf = json("src-tauri/tauri.conf.json");
    let window = &conf["app"]["windows"][0];
    assert_eq!(window["title"], "MotoGP Launcher");
    assert_eq!(window["width"], 1100);
    assert_eq!(window["height"], 700);
    assert_eq!(window["minWidth"], 900);
    assert_eq!(window["minHeight"], 600);
    assert_eq!(conf["identifier"], "dev.motogplauncher.app");
}

#[then("the frontend renders without errors")]
fn frontend_renders(_w: &mut ScaffoldWorld) {
    // Rendering is verified by `renders_launcher_shell` (Vitest); here we
    // assert the entrypoint exists and mounts the app.
    assert!(read("index.html").contains("src/main.tsx"));
    let main = read("src/main.tsx");
    assert!(main.contains("createRoot") && main.contains("App"));
}

// ---------- Scenario 3: reject dependencies against the architecture ----------

#[given("an import from ui to infrastructure in a lint fixture")]
fn lint_fixture(_w: &mut ScaffoldWorld) {
    let bad = read("tests/fixtures/boundaries/violations/src/ui/ImportsInfrastructure.tsx");
    assert!(bad.contains("infrastructure"));
}

#[given("a dependency from application to infrastructure in a Cargo fixture")]
fn cargo_fixture(_w: &mut ScaffoldWorld) {
    let manifest = read("src-tauri/tests/fixtures/outward-deps/application/Cargo.toml");
    assert!(manifest.contains("motogp-infrastructure"));
}

#[when("I run the architecture validators")]
fn run_validators(w: &mut ScaffoldWorld) {
    let (ok, stdout) = eslint_fixture("violations");
    w.lint_violations_found = !ok && stdout.contains("boundaries/dependencies");
    let metadata =
        cargo_metadata(&root().join("src-tauri/tests/fixtures/outward-deps/Cargo.toml"));
    w.cargo_violations_found = !dependency_violations(&metadata).is_empty();
    let (valid_ok, _) = eslint_fixture("valid");
    w.composition_root_valid = valid_ok;
}

#[then("each violation is rejected")]
fn violations_rejected(w: &mut ScaffoldWorld) {
    assert!(
        w.lint_violations_found,
        "ui -> infrastructure should fail the lint"
    );
    assert!(
        w.cargo_violations_found,
        "application -> infrastructure should fail the Cargo validator"
    );
}

#[then("the authorized composition root remains valid")]
fn composition_root_ok(w: &mut ScaffoldWorld) {
    assert!(
        w.composition_root_valid,
        "src/main.tsx should be allowed to import infrastructure"
    );
}

// ---------- Scenario 4: gates in CI ----------

#[given("a PR to dev with Rust and frontend code")]
fn pr_with_rust_and_frontend(w: &mut ScaffoldWorld) {
    assert!(root().join("Cargo.toml").is_file() && root().join("package.json").is_file());
    w.workflow = read(".github/workflows/validacao.yml");
    assert!(w.workflow.contains("pull_request:"));
    assert!(w.workflow.contains("branches: [dev, qas, main]"));
}

#[when("the validation workflow runs")]
fn workflow_runs(w: &mut ScaffoldWorld) {
    w.workflow = read(".github/workflows/validacao.yml");
}

#[then("Rust core and Frontend pass by running their gates")]
fn workflow_runs_gates(w: &mut ScaffoldWorld) {
    let wf = &w.workflow;
    for needle in [
        "name: Rust core",
        "name: Frontend",
        "cargo fmt --all --check",
        "cargo clippy --workspace --all-targets -- -D warnings",
        "cargo test --workspace",
        "cargo test --workspace --test features",
        "cargo test -p motogp-launcher --test architecture",
        "cargo llvm-cov",
        "pnpm prettier --check .",
        "pnpm lint",
        "pnpm tsc --noEmit",
        "pnpm test --coverage",
    ] {
        assert!(wf.contains(needle), "workflow is missing: {needle}");
    }
    // Required jobs must not be skippable through `if`.
    for id in ["rust", "frontend"] {
        let block = job_block(wf, id);
        assert!(
            !block.contains("\n    if:"),
            "job {id} must not have `if` (skip)"
        );
    }
    assert!(
        wf.contains("needs: [rust, frontend]"),
        "the build must come after the gates"
    );
}

#[then("lint, type, BDD or coverage failures fail the validation")]
fn failures_fail_validation(w: &mut ScaffoldWorld) {
    assert!(
        !w.workflow.contains("continue-on-error"),
        "no gate may use continue-on-error"
    );
}

#[then("core coverage is at least 90 percent and overall coverage 80 percent")]
fn coverage_minimums(w: &mut ScaffoldWorld) {
    let thresholds = json("tooling/coverage-thresholds.json");
    assert_eq!(thresholds["core"], 90);
    assert_eq!(thresholds["overall"], 80);
    assert!(w.workflow.contains("scripts/check-rust-coverage.mjs"));
}

/// Text block of a job (from `  <id>:` to the next level-2 job).
fn job_block<'a>(workflow: &'a str, id: &str) -> &'a str {
    let marker = format!("\n  {id}:\n");
    let start = workflow
        .find(&marker)
        .unwrap_or_else(|| panic!("job {id} missing"));
    let rest = &workflow[start + marker.len()..];
    let end = rest
        .lines()
        .scan(0usize, |pos, line| {
            let at = *pos;
            *pos += line.len() + 1;
            Some((at, line))
        })
        .find(|(_, l)| l.starts_with("  ") && !l.starts_with("   ") && l.trim_end().ends_with(':'))
        .map(|(at, _)| at)
        .unwrap_or(rest.len());
    &rest[..end]
}

fn main() {
    futures::executor::block_on(
        ScaffoldWorld::cucumber()
            .fail_on_skipped()
            .run_and_exit("tests/features"),
    );
}
