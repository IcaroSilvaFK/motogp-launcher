Feature: Library and profile actions are locked during a game session

  Background:
    Given the library has the mods "Fast Bike" and "Rain Tyres"
    And a profile "Mine" exists
    And a game session is running

  Scenario: Library Remove buttons are locked
    When the user opens the Library
    Then every Remove button is marked as disabled
    And each Remove button explains that mods cannot be removed while the game is running
    When the user clicks Remove for "Rain Tyres"
    Then no mod is removed

  Scenario: Profile management is locked
    When the user opens Profiles and selects "Mine"
    Then the Create, Rename, Delete and Save actions are marked as disabled
    And each explains that profile changes are locked while the game is running
    When the user tries to create, rename, delete and save a profile
    Then no profile change is sent

  Scenario: Actions unlock when the session ends
    Given the game session has ended
    When the user clicks Remove for "Rain Tyres"
    Then "Rain Tyres" is removed

  Scenario: The new error code is translated
    Given the language is "pt-BR"
    Then the error "library_locked_by_session" has a Portuguese message
