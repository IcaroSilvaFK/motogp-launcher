Feature: Start the project with boundaries and mandatory validation

  Scenario: Generate the scaffold preserving the project
    Given the project contains versioned documents and workflows
    When the scaffold is integrated into the current root
    Then the documents and the history are preserved
    And the workspace contains domain, application, infrastructure and src-tauri
    And the workspace and frontend tests finish successfully

  # The real Windows window-opening smoke is manual (docs/workflow-validacao.md).
  Scenario: Validate the Windows application configuration
    Given the Tauri scaffold is available
    When I inspect the application startup configuration
    Then the MotoGP Launcher window is configured with the specified dimensions
    And the frontend entrypoint is configured

  Scenario: Reject dependencies that violate the architecture
    Given an import from ui to infrastructure in a lint fixture
    And a dependency from application to infrastructure in a Cargo fixture
    When I run the architecture validators
    Then each violation is rejected
    And the authorized composition root remains valid

  Scenario: Run all gates in CI
    Given a PR to dev with Rust and frontend code
    When the validation workflow runs
    Then Rust core and Frontend pass by running their gates
    And lint, type, BDD or coverage failures fail the validation
    And core coverage is at least 90 percent and overall coverage 80 percent
