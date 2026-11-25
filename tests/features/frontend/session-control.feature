Feature: Session control in the status bar
  Scenario: Play turns into Stop while the game runs
    Given the game session is running since 14:05
    Then the status bar shows "Running since 14:05"
    And the main action is "Stop" instead of "Play"

  Scenario: Stop asks for confirmation
    Given the game session is running
    When the user clicks "Stop"
    Then a confirmation is shown
    And the session is not stopped until the user confirms

  Scenario: Confirming Stop stops the session and shows the cleanup
    Given the game session is running
    When the user stops the session and confirms
    Then the backend is asked to stop the session
    And the status bar shows "Stopping the game…"
    When the backend reports "cleaning-up"
    Then the status bar shows "Cleaning up…"
    When the backend reports the session ended with 2 files cleaned
    Then the main action is "Play" again

  Scenario: A failed stop is reported and Stop stays available
    Given the game session is running
    When stopping fails with "stop_failed"
    Then an error explains the game could not be stopped
    And the main action is still "Stop"

  Scenario: The session survives a page reload
    Given the backend reports a running session since 14:05
    When the launcher loads
    Then the status bar shows "Running since 14:05"
    And Play is not offered
