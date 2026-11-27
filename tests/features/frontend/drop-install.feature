Feature: Drop to install feedback
  Dragging files over the Library shows a full-window drop overlay, and dropped files that are not
  mod archives are named in a notice while the valid ones still install (#152).

  Scenario: Dragging a file shows the drop overlay
    Given the launcher shows the Library
    When a file is dragged over the launcher window
    Then a "Drop to install" overlay is shown

  Scenario: Leaving the window hides the overlay
    Given the launcher shows the Library
    And a file is being dragged over the launcher window
    When the drag leaves the window
    Then the overlay is hidden

  Scenario: Dropping the file hides the overlay
    Given the launcher shows the Library
    And a file is being dragged over the launcher window
    When the file is dropped
    Then the overlay is hidden

  Scenario: Esc hides the overlay
    Given the launcher shows the Library
    And a file is being dragged over the launcher window
    When the user presses Esc
    Then the overlay is hidden

  Scenario: Non-zip files are rejected with a message
    Given the launcher shows the Library
    When "readme.txt" and "livery.zip" are dropped
    Then "livery.zip" is installed
    And a notice says "readme.txt" is not a mod archive

  Scenario: Only rejected files dropped
    Given the launcher shows the Library
    When only "readme.txt" is dropped
    Then nothing is installed
    And a notice says "readme.txt" is not a mod archive
