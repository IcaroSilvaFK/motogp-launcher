Feature: Classify launcher data for storage usage
  Scenario: Imported mods are counted as mods
    Given an entry "library" that is a directory
    Then it is classified as "mods"

  Scenario: Launcher state files are counted as launcher data
    Given the entries "config.json", "PROFILES.JSON", "deployed.json" and "library.json.tmp" that are files
    Then each one is classified as "launcher data"

  Scenario: Anything else is counted as other
    Given an entry "notes.txt" that is a file and an entry "library" that is a file
    Then each one is classified as "other"

  Scenario: The total is the sum of the categories
    Given a usage with mods 300 bytes, launcher data 20 bytes and other 5 bytes
    Then the total is 325 bytes
    And the categories are listed as mods, launcher data, other
