Feature: Measure the data folder on disk
  Scenario: Sizes are measured recursively in a path with accents
    Given a data folder under "Usuário Padrão" with "library/a/mod.pak" of 300 bytes, "library/b/x.pak" of 200 bytes and "config.json" of 12 bytes
    When the folder is scanned
    Then "library" is a directory of 500 bytes
    And "config.json" is a file of 12 bytes
    And the free space of the drive is greater than 0

  Scenario: A missing data folder is empty, not an error
    Given the data folder does not exist yet
    When the folder is scanned
    Then the scan returns no entries

  Scenario: Links are not followed
    Given the data folder contains a directory link to a folder with 1000 bytes
    When the folder is scanned
    Then the bytes behind the link are not counted
