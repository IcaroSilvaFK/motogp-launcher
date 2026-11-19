Feature: Identify the start and end of the session
  Scenario: Build the URL with a detected App ID
    Given a game with App ID 123
    When I prepare its launch
    Then I get "steam://rungameid/123"

  Scenario: Refuse to launch without an App ID
    Given a game without an App ID
    When I prepare its launch
    Then I get "game App ID not found; open the game from Steam once"

  Scenario: End after two consecutive absences
    Given the first poll finds the game and emits Started
    When the next poll does not find the process
    Then no end is emitted
    When the following poll also does not find the process
    Then Ended is emitted exactly once

  Scenario: Reset absences when the process reappears
    Given the poll sequence present, absent, present, absent
    When I advance the state machine
    Then Started is emitted on the first presence
    And Ended is not emitted

  Scenario: Report a game that never started
    Given no poll has found the process
    When the elapsed time reaches 119 seconds
    Then no end is emitted
    When the time reaches 120 seconds
    Then NeverStarted is emitted exactly once
