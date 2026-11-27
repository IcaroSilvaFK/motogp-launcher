Feature: Library sorting and filtering

  Scenario: Sorting by name
    Given the mods "Zeta", "Alpha" and "Mid" are installed
    When the user sorts by name
    Then the list reads "Alpha", "Mid", "Zeta"

  Scenario: Sorting by size
    Given the mods "Zeta", "Alpha" and "Mid" are installed
    When the user sorts by size
    Then the list reads "Alpha", "Mid", "Zeta" from the largest to the smallest

  Scenario: The sort choice is remembered
    Given the user sorted by size
    When the launcher is reopened
    Then the Library is still sorted by size

  Scenario: Showing only conflicting mods
    Given "A" and "B" conflict and "C" does not
    When the user turns on "Has conflicts"
    Then only "A" and "B" are listed
