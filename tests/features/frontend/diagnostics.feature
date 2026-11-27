Feature: Diagnostics
  Users can attach the launcher log and a short diagnostics text to a bug report.

  Scenario: Copy diagnostics
    Given the launcher version is "0.1.0", the OS is "Windows 11" and a game is configured
    When the user clicks "Copy diagnostics" in About
    Then a text with the app version, OS, game folder, mod count, active profile and recent log lines is copied
    And "Copied" is shown

  Scenario: Diagnostics without a game folder
    Given no game folder is configured
    When the user clicks "Copy diagnostics" in About
    Then the copied text says the game folder was not detected

  Scenario: Copying fails
    Given the clipboard is not available
    When the user clicks "Copy diagnostics" in About
    Then an error is shown instead of "Copied"

  Scenario: Open logs folder
    Given the Settings screen is open
    When the user clicks "Open logs folder" in Settings > Storage
    Then the folder holding the log files opens

  Scenario: No secrets in diagnostics
    Given the backend already removed the session token from the log lines
    When the user clicks "Copy diagnostics" in About
    Then the copied text never contains the community session token

  Scenario: The labels follow the language
    Given the language is Portuguese
    Then the diagnostics actions are labeled in Portuguese
