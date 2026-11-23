# language: en
Feature: Recover a stale session through the Play service

  Background:
    Given a configured game and a manifest that points to a previous game install

  Scenario: Forget the old session and play
    When the user forgets the old session
    Then the manifest is empty
    And the old files are untouched
    And Play succeeds

  Scenario: Clean the old folder with the confirmed folder
    When the user confirms cleaning the old folder
    Then only the files of the manifest are removed from the old folder
    And the manifest is empty

  Scenario: Cleaning with a different folder is refused
    When the user confirms cleaning a different folder
    Then the cleanup is refused as unauthorized
    And nothing is removed

  Scenario: The current session cannot be forgotten
    Given the manifest points to the current game install
    When the user forgets the old session
    Then the recovery is refused as not stale
    And the manifest is kept

  Scenario: A vanished folder is reported as vanished
    Given the old folder no longer exists
    Then the deploy session is vanished

  Scenario: Recovery is refused while a session is active
    Given a game session is active
    When the user forgets the old session
    Then the recovery is refused because a session is running
    And the manifest is kept

  Scenario: Recovery is refused while the previous game is running
    Given the previous game is still running
    When the user forgets the old session
    Then the recovery is refused because the game is running
    And the manifest is kept

  Scenario: Forgetting unblocks Play after a stale startup failure
    Given the startup recovery failed because of the stale manifest
    When the user forgets the old session
    Then the recovery status is recovered
    And Play succeeds

  Scenario: Recovery needs a configured game
    Given no game is configured
    When the user forgets the old session
    Then the recovery is refused because no game is configured
