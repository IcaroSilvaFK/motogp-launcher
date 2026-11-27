Feature: Only the content area scrolls

  The shell is a fixed window-sized grid: the title bar, sidebar and status bar stay put and only
  the content area scrolls, on every screen.

  Scenario: The content area is the containing block of out-of-flow content
    Given the launcher stylesheet
    Then the content area clips absolutely positioned descendants
    And the document itself does not scroll

  Scenario: Jumping to a settings section scrolls only the content area
    Given the launcher shows Settings
    When the user picks the "Storage" section
    Then the content area scrolls to the Storage section
    And the document is not scrolled
