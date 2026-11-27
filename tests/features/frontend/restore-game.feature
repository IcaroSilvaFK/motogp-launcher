Feature: Restore the original game files from Settings

  Scenario: Storage shows that game files are safe and how many mod files are deployed
    Given 3 mod files are deployed
    When the user opens Settings > Storage
    Then the Game files row says the original game files are never changed
    And it shows "3 mod files deployed"

  Scenario: Restoring the original game
    Given 3 mod files are deployed
    When the user chooses "Restore original game files"
    Then the backend is asked to restore the game files
    And a notice says how many files were removed

  Scenario: Restore is blocked during a session
    Given the game is running
    Then "Restore original game files" is disabled with the reason

  Scenario: Restore is blocked while an old session is pending
    Given a stale session from a previous install is pending
    Then "Restore original game files" is disabled with a reason pointing to the old-session notice

  Scenario: The section is translated
    Given the language is "pt-BR"
    Then the Game files row and its reasons are in Portuguese
