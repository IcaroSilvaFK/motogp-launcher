# language: en
Feature: Maintain ordered profiles

  Scenario: Seed missing or empty profiles
    Given there are no stored profiles
    When I list the profiles
    Then I receive only Vanilla with ID "vanilla" and no mods
    And that profile is persisted

  Scenario: Create and list a profile
    Given Vanilla already exists
    When I create "Liveries 2025"
    Then the list contains two profiles
    And the new profile is named "Liveries 2025" and has its own ID

  Scenario: Preserve the mod order
    Given an existing profile
    When I set its mods to "b", "a", "c"
    Then reading it returns exactly "b", "a", "c"

  Scenario: Delete an ordinary profile
    Given a profile other than Vanilla
    When I delete it
    Then it no longer appears in the persisted list

  Scenario: Remove references from all profiles
    Given several profiles contain the mod "a"
    When I remove the references to the mod "a"
    Then no profile contains "a"
    And the order of the other mods is preserved

  Scenario: Reject a blank name
    Given a name made only of spaces
    When I try to create or rename a profile with that name
    Then I receive the error "empty profile name"
    And the previous data is preserved

  Scenario: Protect Vanilla in the backend
    Given the profile with ID "vanilla"
    When I try to delete it through the use case
    Then the deletion is refused
    And Vanilla is still saved

  Scenario: Rename a profile and handle an unknown ID
    Given an existing profile
    When I rename it to "Race"
    Then the updated name is persisted
    When I look up an unknown ID
    Then I receive the error "profile not found"

  Scenario: Profile edits are refused while a game session is active
    Given a profile "Race" exists
    And a game session is active
    When I try to rename, delete, activate, create or change the mods of a profile
    Then every attempt fails with "session active"
    And the stored profiles and the active profile are unchanged

  Scenario: Profiles can be read while a game session is active
    Given a profile "Race" exists
    And a game session is active
    When I list the profiles
    Then "Race" is listed

  Scenario: Profile edits work again after the session ends
    Given a profile "Race" exists
    And a game session was active and has ended
    When I change the mods of "Race" to "m1"
    Then "Race" has the mods "m1"
