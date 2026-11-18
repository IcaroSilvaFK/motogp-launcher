Feature: Launcher shell and frontend boundaries

  Scenario: Render the launcher shell
    Given the launcher started in the WebView
    When the interface is rendered
    Then I see the accessible MotoGP Launcher shell
    # Test: src/ui/__tests__/scaffold.test.tsx (renders_launcher_shell)

  Scenario: Reject an import from ui to infrastructure
    Given an import from ui to infrastructure in a lint fixture
    When I run ESLint with eslint-plugin-boundaries
    Then the violation is rejected
    And the src/main.tsx composition root remains valid
    # Test: tooling/__tests__/architecture.test.ts (rejects_ui_importing_infrastructure)

  Scenario: Insufficient coverage fails the gate
    Given a coverage report below the thresholds
    When the coverage gate is evaluated
    Then the validation fails
    # Test: tooling/__tests__/coverage.test.ts (coverage_thresholds_reject_insufficient_coverage)
