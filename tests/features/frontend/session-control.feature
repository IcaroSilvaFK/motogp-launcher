Feature: Session control in the status bar
  Scenario: Play turns into Stop while the game runs
    Given the game session is running since 14:05
    Then the status bar shows "Running since 2:05 PM"
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
    Then the status bar shows "Running since 2:05 PM"
    And Play is not offered

  Scenario: The Stop confirmation focuses Cancel
    Given the game session is running
    When the user clicks "Stop"
    Then the focus is on "Cancel"
    When the user presses Enter
    Then the session is not stopped
    And the main action is "Stop"

  Scenario: Stop shows the busy indicator while the game is starting
    Given the game is starting
    Then the main action is "Stop" and it is marked busy
    When the backend reports the session is running
    Then the main action "Stop" is no longer marked busy

  Scenario: Running since follows the UI language
    Given the game session is running since 14:05
    When the language is "en"
    Then the status bar shows "Running since 2:05 PM"
    When the language is "pt-BR"
    Then the status bar shows the Portuguese "running since" text with "14:05"
