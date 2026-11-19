Feature: Track the game session
  Scenario: Use the defined tracking interval
    Given a controlled clock and process inspector
    When I track a session
    Then the queries are scheduled at 2-second intervals
