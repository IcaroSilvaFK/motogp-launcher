Feature: Expose use cases and recover sessions through the launcher
  Scenario: Resolve mods in order ignoring missing references
    Given a profile with IDs "b", "gone", "a" and a library with A and B
    When I resolve the profile mods
    Then I get B and A in that order

  Scenario: Recover a previous deployment on startup
    Given a pending manifest and a manual file in ~mods
    When the launcher starts
    Then the recovery cleans only the files deployed by the launcher
    And records the amount removed before accepting a new session

  Scenario: Prevent a simultaneous session
    Given a session starting or running
    When I receive another Play request
    Then I return "the game is already running"
    And I neither deploy nor open the game again

  Scenario: Validate prerequisites before deploy
    Given a game not configured or without an App ID
    When I receive the Play request
    Then I return the corresponding error
    And no file or directory is created in ~mods
    And a future attempt remains possible

  Scenario: Finish a session with cleanup and event
    Given a session with deployed files
    When the watcher signals Ended or NeverStarted
    Then the cleanup runs
    And the session event contains the state and the amount actually removed
    And a new session can be started after the complete cleanup

  Scenario: Emit start without cleaning prematurely
    Given a session with deployed mods
    When the watcher signals Started
    Then I emit session with state "started"
    And the files remain deployed

  Scenario: Compensate a failure to open Steam
    Given mods deployed for a session attempt
    When the opener adapter returns an error
    Then I try to clean only the files of that session
    And I report the error without keeping the lock after a successful compensation

  Scenario: Do not hide a recovery failure
    Given the manifest cleanup fails on startup or at the end
    When the operation returns
    Then I report the failure and retain the recoverable pending items
    And I do not announce a clean session nor allow an unsafe new deploy

  Scenario: Keep profiles consistent when removing data
    Given a mod referenced in profiles and a regular active profile
    When I remove the mod and delete the active profile
    Then the references to the mod are removed from all profiles
    And the active profile becomes Vanilla

  Scenario: Never clean or redeploy while the game is still running
    Given a manifest left by a previous run and its game still running
    When the launcher starts
    Then the files stay deployed and tracking resumes
    And Play is refused until the game ends and the cleanup completes

  Scenario: Release the session lock when the worker fails
    Given a session whose background worker panics
    When the worker runs
    Then the lock is released and the cleanup stays pending
    And the next Play retries the cleanup before deploying

  Scenario: Keep the startup recovery result readable
    Given a manifest whose cleanup fails on startup
    When the interface reads the recovery status after subscribing
    Then it still sees the failure and its message

  Scenario: Removing a mod from the library is refused while a session is active
    Given the library has the mod "m1" referenced by the profile "Race"
    And a game session is active
    When I remove the mod "m1"
    Then the removal fails with "session active"
    And "m1" is still in the library
    And "Race" still has the mod "m1"

  Scenario: Removing a mod works again after the session ends
    Given the library has the mod "m1" referenced by the profile "Race"
    And a game session was active and has ended
    When I remove the mod "m1"
    Then "m1" is no longer in the library
    And "Race" has no mods
