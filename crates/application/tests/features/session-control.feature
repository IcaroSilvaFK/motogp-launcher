Feature: Control the running game session
  Scenario: The current session is idle before any Play
    Given startup recovery has run and nothing is deployed
    Then the current session phase is "idle"

  Scenario: The current session is reported while the game runs
    Given a session was started and the game process appeared at 14:05
    Then the current session phase is "running" since 14:05

  Scenario: The cleanup phase is published before the session ends
    Given a running session
    When the game exits
    Then the events are "started", "cleaning-up" and "ended" in that order

  Scenario: Stop ends the game and runs the normal cleanup
    Given a running session with 3 deployed files
    When the user stops the session
    Then the game processes inside the install root are terminated
    And the session ends with 3 files cleaned
    And the current session phase is "idle"

  Scenario: Stop while the game is still starting does not wait for the deadline
    Given a session that has not seen the game yet
    When the user stops the session
    Then the session ends without waiting 120 seconds and the deployed files are cleaned

  Scenario: Stop without a session is refused
    Given no session is active
    When the user stops the session
    Then the error is "no active session"

  Scenario: A failed stop keeps the session consistent
    Given a running session
    And the game processes cannot be terminated
    When the user stops the session
    Then the error is "stop failed"
    And the current session phase is "running"
    And no file is cleaned
