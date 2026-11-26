Feature: One-click install

  Scenario: Install leaves the mod ready to play
    Given the active profile is "Default"
    When the user installs a catalog mod
    Then the mod is in the library and enabled in "Default"
    And the install button shows "Installed and enabled"
    And the toast offers "Undo" and "Open in Library"

  Scenario: A conflicting mod stays disabled
    Given an enabled mod replaces the same file
    When the user installs a catalog mod that replaces it too
    Then the new mod is installed but disabled
    And the user is told which mod conflicts

  Scenario: Auto-enable turned off
    Given "Enable mods after installing from Discover" is off
    When the user installs a catalog mod
    Then it is installed but not enabled

  Scenario: Undo disables the mod again
    Given the active profile is "Default"
    When the user installs a catalog mod
    And the user clicks "Undo" in the toast
    Then the mod stays in the library but is no longer enabled in "Default"
