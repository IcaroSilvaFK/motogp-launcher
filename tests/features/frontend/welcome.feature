Feature: First-run welcome
  Scenario: First launch shows the welcome
    Given the launcher has never been opened
    When the launcher opens
    Then the welcome flow is shown at step 1 of 3
    And the welcome title has the focus

  Scenario: The game is detected
    Given the launcher has never been opened
    And the game folder can be detected
    When the launcher opens
    Then step 1 shows the detected path and a "Looks right" action
    When I confirm the detected folder
    Then the detected folder is configured
    And step 2 explains how mods work

  Scenario: The game is not detected
    Given the launcher has never been opened
    And no game folder can be detected
    When the launcher opens
    Then step 1 asks me to pick the game folder
    When I pick a folder that is not the game
    Then step 1 shows why the folder was rejected

  Scenario: Finishing goes to Discover and does not show the welcome again
    Given the launcher has never been opened
    And a game is configured
    When the launcher opens
    And I go through the three steps and choose "Go to Discover"
    Then the welcome is closed and Discover is shown
    And the welcome is remembered as seen
    When the launcher opens again
    Then the welcome flow is not shown

  Scenario: Skipping does not show the welcome again and Settings offers it
    Given the launcher has never been opened
    When the launcher opens
    And I skip the welcome
    Then the welcome is remembered as seen
    When I open Settings and choose "Show welcome again"
    Then the welcome flow is shown at step 1 of 3

  Scenario: Escape skips the welcome
    Given the launcher has never been opened
    When the launcher opens
    And I press Escape
    Then the welcome is closed
    And the welcome is remembered as seen

  Scenario: A broken store does not block the launcher
    Given a welcome store that fails to read and to save
    When the launcher opens
    Then the welcome flow is not shown
