# language: en
Feature: Restore the original game files on demand

  Scenario: Restoring removes the deployed files of the current install
    Given a configured game with 2 deployed files
    When the user restores the original game files
    Then 2 files are removed and the manifest is empty

  Scenario: Restoring retries a failed cleanup and unblocks Play
    Given the startup cleanup failed
    When the user restores the original game files
    Then the recovery status is recovered
    And Play succeeds

  Scenario: Restore is refused while a session is active
    Given a configured game with 2 deployed files
    And a game session is active
    When the user restores the original game files
    Then it is refused because a session is running
    And nothing is removed

  Scenario: Restore is refused while the previous game is running
    Given the previous game is still running
    When the user restores the original game files
    Then it is refused because the game is still running
