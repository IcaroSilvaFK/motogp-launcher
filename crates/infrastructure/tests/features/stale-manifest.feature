# language: en
Feature: Recover from a manifest of a previous game install on the real file system

  Background:
    Given the authorized ~mods of the current game install

  Scenario: Detect a manifest of a previous game install
    Given a deploy manifest that points to a previous game install
    When I inspect the deploy session
    Then it is reported as stale with the old folder and its tracked file count
    And nothing is changed on disk or in the manifest

  Scenario: Forget the old session
    Given a deploy manifest that points to a previous game install
    When the user chooses to forget the old session
    Then the manifest is cleared
    And no file is deleted

  Scenario: Clean the old folder after confirmation
    Given a deploy manifest that points to a previous game install
    When the user confirms cleaning the old folder
    Then only files proven to be ours are deleted from that folder
    And the manifest is cleared

  Scenario: A foreign file in the old folder is never deleted
    Given a deploy manifest that points to a previous game install
    And the user file "meu_mod.pak" in the old folder
    When the user confirms cleaning the old folder
    Then "meu_mod.pak" keeps the same bytes

  Scenario: A file modified since the deploy is not deleted
    Given a deploy manifest that points to a previous game install
    And a tracked file in the old folder was modified after the deploy
    When the user confirms cleaning the old folder
    Then the modified file keeps its new bytes
    And the manifest is cleared

  Scenario: The confirmation must name the recorded folder
    Given a deploy manifest that points to a previous game install
    When the user confirms cleaning a different folder
    Then the cleanup is refused as unauthorized
    And no file is deleted
    And the manifest is kept

  Scenario: Refuse to clean a folder that is not a ~mods folder
    Given a deploy manifest that points to a folder not named "~mods"
    When the user confirms cleaning the old folder
    Then the cleanup is refused as unauthorized
    And no file is deleted

  Scenario: The old folder no longer exists
    Given a deploy manifest that points to a folder that was deleted
    When I inspect the deploy session
    Then it is reported as vanished
    When the user confirms cleaning the old folder
    Then the manifest is cleared without an error

  Scenario: A manifest of the current install keeps the normal behaviour
    Given a deploy manifest that points to the authorized ~mods
    When I inspect the deploy session
    Then it is reported as current
    And forgetting it is refused as not stale and the manifest is kept
    And deploying cleans it as before

  Scenario: Deploy works again after recovering
    Given a deploy manifest that points to a previous game install
    When the user chooses to forget the old session
    And I deploy a profile into the authorized ~mods
    Then the deploy succeeds
