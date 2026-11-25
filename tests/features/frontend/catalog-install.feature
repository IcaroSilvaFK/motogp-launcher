Feature: Install button for catalog entries
  Scenario: Download progress is shown
    Given an install of "rebalanced-ai" reports 42% downloaded
    Then the install button shows "42%"

  Scenario: Installed entries appear in the library
    When the install of "rebalanced-ai" finishes
    Then the install button shows "Installed"
    And the library is reloaded

  Scenario: A failed install can be retried
    Given the install of "rebalanced-ai" failed with a hash mismatch
    Then the translated hash mismatch message is shown
    When the user clicks "Retry"
    Then the install starts again

  Scenario: Install is disabled during a session
    Given a game session is running
    Then the install button is disabled
