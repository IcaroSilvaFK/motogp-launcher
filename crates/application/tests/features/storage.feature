Feature: Storage usage
  Scenario: Usage is grouped per category
    Given the data folder "C:/Users/Usuário Padrão/AppData/Roaming/motogp-launcher" holds "library" with 1000 bytes, "config.json" with 10 bytes, "profiles.json" with 20 bytes and "crash.dmp" with 5 bytes
    And the drive has 5000 bytes free
    When the storage usage is requested
    Then mods use 1000 bytes, launcher data 30 bytes and other 5 bytes
    And the total used is 1035 bytes
    And the free space is 5000 bytes
    And the data folder is reported with its accented path

  Scenario: An empty data folder reports zero in every category
    Given the data folder holds nothing
    When the storage usage is requested
    Then every category uses 0 bytes and the total used is 0 bytes

  Scenario: Free space that cannot be read does not fail the usage
    Given the free space of the drive cannot be read
    When the storage usage is requested
    Then the usage is returned with the free space unknown

  Scenario: A data folder that cannot be read is an error
    Given the data folder cannot be scanned
    When the storage usage is requested
    Then a read error naming the data folder is returned

  Scenario: Open folder opens the data folder
    When the user opens the data folder
    Then the folder opener receives exactly the data folder path
