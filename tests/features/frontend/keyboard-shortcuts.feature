Feature: Keyboard shortcuts
  Scenario: Esc leaves the mod page
    Given the user opened a mod page from Discover and focus is on the page body
    When they press Esc
    Then Discover is shown with the mod card focused

  Scenario: Slash focuses search
    Given the Library is open
    When the user presses "/"
    Then the Library search field is focused

  Scenario: Ctrl+F focuses search
    Given Discover is open
    When the user presses Ctrl+F
    Then the Discover search field is focused

  Scenario: Shortcuts are ignored while typing
    Given the focus is in the Library search field
    When the user types "/"
    Then a "/" is typed and the focus stays put

  Scenario: Esc is ignored while typing on the mod page
    Given a mod page is open and focus is in a text field
    When the user presses Esc
    Then the mod page stays open

  Scenario: Switching sections
    Given the Library is open
    When the user presses Ctrl+2
    Then the second sidebar section opens

  Scenario: Settings lists the shortcuts
    Given the Settings screen is open
    Then the keyboard shortcuts are listed in About
