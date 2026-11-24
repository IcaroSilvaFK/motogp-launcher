Feature: Game artwork in the status bar
  Scenario: Show the Steam artwork of the configured game
    Given a configured Steam game with cached artwork
    When the launcher opens
    Then the status bar shows the artwork as a decorative image

  Scenario: Fall back to the generic icon when there is no artwork
    Given a configured game without cached artwork
    When the launcher opens
    Then the status bar shows the generic game icon

  Scenario: Fall back when the image cannot be displayed
    Given the artwork fails to load
    Then the status bar shows the generic game icon

  Scenario: No game configured
    Given no game is configured
    Then the status bar shows the generic game icon
