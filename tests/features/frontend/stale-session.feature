Feature: Recover a stale session from the interface
  Background:
    Given the manifest points to a previous game install

  Scenario: The user forgets the old session
    When the user chooses "Forget the old session"
    Then the notice disappears
    And Play is enabled again

  Scenario: The user cleans the old folder
    When the user confirms cleaning the old folder
    Then only files proven to be ours are removed
    And the notice disappears

  Scenario: Play is blocked while a stale session is pending
    Then a notice names the old folder
    And Play is disabled

  Scenario: Cancelling the confirmation keeps everything
    When the user chooses "Clean the old folder" and cancels
    Then nothing is removed
    And the notice is still shown

  Scenario: Cleaning is refused when the confirmed folder does not match
    When cleaning is confirmed with a different folder
    Then the error "deploy_unauthorized_directory" is shown
    And nothing is removed and the notice is still shown

  Scenario: Switching the game folder makes the session stale
    Given a session deployed in the current game folder
    When the user configures another game folder
    Then the stale notice appears

  Scenario: A vanished old folder needs no recovery
    Given the manifest points to a folder that no longer exists
    Then no stale notice is shown
    And Play is enabled

  Scenario: A stale manifest at startup unblocks Play once resolved
    Given the startup recovery failed because of the stale manifest
    Then the stale notice is shown instead of the generic recovery error
    When the user forgets the old session
    Then Play deploys normally
