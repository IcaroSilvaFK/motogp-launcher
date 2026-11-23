Feature: Launcher version and manual update check

  Scenario: The status bar shows the launcher version
    Given the launcher version is "0.1.0"
    When the launcher opens
    Then the status bar shows "v0.1.0" next to the game

  Scenario: Settings About shows the launcher version
    Given the launcher version is "0.1.0"
    When the user opens Settings and the About section
    Then the About section shows the version "0.1.0"

  Scenario: The version stays hidden when it cannot be read
    Given reading the launcher version fails
    When the launcher opens
    Then the status bar shows no version
    And the launcher stays usable

  Scenario: Checking manually when already up to date
    Given the launcher version is "0.1.0" and no newer release exists
    When the user clicks "Check for updates" in About
    Then the button is disabled and reads "Checking…" while the check runs
    And About says the launcher is up to date

  Scenario: Checking manually finds a newer version
    Given a release "0.2.0" is published
    When the user clicks "Check for updates" in About
    Then About says version "0.2.0" is available
    And an install action is offered that installs through the update controller

  Scenario: A manual check that fails shows a translated error
    Given the update server cannot be reached
    And the language is Portuguese (Brazil)
    When the user clicks "Check for updates" in About
    Then About shows the translated network error
    And the raw error message is not shown

  Scenario: A failed background check does not claim the launcher is up to date
    Given the startup check failed silently
    When the user opens the About section
    Then About does not say the launcher is up to date

  Scenario: Install from About is blocked during a game session
    Given a release "0.2.0" is available
    And a game session is active
    Then the install action in About is disabled with the session hint
