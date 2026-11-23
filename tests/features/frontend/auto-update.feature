Feature: Update the installed launcher through the interface
  Scenario: Find and install a newer version
    Given the installed app is on version "1.0.0"
    And the service reports the release "1.1.0" with notes and a valid signature
    When the app starts
    Then I see "New version 1.1.0 available" and the notes
    When I click Update with no pending game session
    Then I see the download progress
    And the installation completes before the restart request
    And the app restarts on version "1.1.0"

  Scenario: Already on the latest version
    Given the installed app and the latest release are on version "1.1.0"
    When the app starts
    Then no update banner is shown
    And no download or restart is requested

  Scenario: Do not install an older version
    Given the app on version "1.10.0" and a feed with "1.9.0"
    When I check for updates
    Then the older release is not offered for installation

  Scenario: Keep the launcher usable offline
    Given the feed query fails due to the network
    When the app starts
    Then no update banner is shown
    And I can still use library, profiles and settings
    And no restart is requested

  Scenario: Stay silent when the update feed is not published
    Given the feed query fails because the release JSON cannot be fetched
    When the app starts
    Then no update banner is shown
    And the failure is only logged to the console

  Scenario: Show only the translated message when the user checks for updates
    Given the feed query fails due to the network
    When I check for updates
    Then I see the translated network error
    And the raw error text from the updater is not shown

  Scenario: Refuse a package with an invalid signature
    Given an update whose package fails the signature verification
    When I try to update it
    Then the installation is refused and the error is visible
    And the app neither restarts nor replaces the installed version

  Scenario: Handle an installation failure
    Given the download finishes but the installation fails
    When the service returns the error
    Then the error is visible and the attempt is no longer in progress
    And the raw error text from the updater is not shown
    And no restart is requested

  Scenario: Avoid simultaneous installs
    Given an update in progress
    When I trigger Update again
    Then there is no second download nor second installation

  Scenario: Protect a game session
    Given a session starting, running or awaiting cleanup
    When an update becomes available
    Then I can see the new version
    But the installation and restart are unavailable until the session is clean

  Scenario: Publish the feed only through the production flow
    Given a push to an environment branch with approved gates
    When the pipeline produces the installers
    Then dev and qas keep NSIS and MSI for 30 days with no production release
    And main publishes the release of the configured version with latest.json and signed artifacts
    And the app configuration contains only the public key
