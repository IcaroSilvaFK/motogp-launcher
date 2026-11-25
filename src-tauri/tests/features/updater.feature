Feature: Publish and verify in-app updates only through the production flow

  # Stays red until the repository owner generates the production signing key
  # pair and puts the PUBLIC key into plugins.updater.pubkey (docs/auto-update.md).
  # updater_config_contains_public_key_and_endpoint
  Scenario: Configure the updater with the production public key
    Given the Tauri application configuration
    Then the updater endpoint is the latest release feed over HTTPS
    And the updater installs in passive mode and creates updater artifacts
    And the updater public key is a real minisign public key without private material

  # updater_capabilities_are_minimal
  Scenario: Grant the window only the updater permissions it needs
    Given the main window capability file
    Then it grants exactly the updater check, the process restart and the title bar window permissions

  # unsigned_or_tampered_update_is_rejected
  Scenario: Reject an unsigned or tampered update package
    Given a test-only public key and a package signed with the matching test key
    When the signature of the untouched package is verified
    Then the package is accepted
    When the signature of a tampered package is verified
    Then the tampered package is rejected
    When a package comes with a garbage signature or none at all
    Then the unsigned package is rejected

  # release_job_requires_successful_validation
  Scenario: Publish the update feed only after validation on main
    Given the validation workflow
    Then the installer build needs the Rust and Frontend gates and runs only on push
    And dev and qas keep the installers as 30-day artifacts without a release
    And only main creates the release with the updater feed
    And the signing keys come only from repository secrets
