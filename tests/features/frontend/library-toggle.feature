Feature: Toggle a mod in the active profile from the library

  Background:
    Given the library has the mods "Fast Bike" and "Rain Tyres"
    And the active profile "Mine" contains only "Fast Bike"

  Scenario: Each library row shows a switch bound to the active profile
    When the user opens the Library
    Then the switch for "Fast Bike" is on
    And the switch for "Rain Tyres" is off

  Scenario: Turning a mod on adds it to the active profile
    When the user turns on the switch for "Rain Tyres"
    Then the active profile is saved with "Fast Bike" and "Rain Tyres" in that order
    And the switch for "Rain Tyres" is on
    And the "In active profile" filter count is 2

  Scenario: Turning a mod off removes it from the active profile
    When the user turns off the switch for "Fast Bike"
    Then the active profile is saved with no mods
    And the "Not in active profile" filter count is 2

  Scenario: The current filter updates immediately
    Given the "In active profile" filter is selected
    When the user turns off the switch for "Fast Bike"
    Then "Fast Bike" is no longer listed

  Scenario: Switches are locked while a game session is running
    Given a game session is running
    Then every switch is marked as disabled
    And each switch explains that profile changes are locked while the game is running
    When the user clicks the switch for "Rain Tyres"
    Then the active profile is not changed

  Scenario: The Vanilla profile cannot receive mods
    Given the active profile is "Vanilla"
    Then every switch is off and marked as disabled
    And each switch explains that Vanilla runs without mods

  Scenario: A failed save shows the error
    Given saving the profile fails
    When the user turns on the switch for "Rain Tyres"
    Then an error notice is shown
    And the switch for "Rain Tyres" is off
