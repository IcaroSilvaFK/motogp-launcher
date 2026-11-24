# language: en
Feature: Persist local data safely

  Scenario: Read a missing configuration
    Given config.json does not exist yet
    When I load the configuration
    Then I receive the default configuration

  Scenario: Save and read in a path with spaces and accents
    # Non-ASCII on purpose: Windows user folders often contain accents (e.g. "Usuário Padrão").
    Given a directory named "Usuário Padrão"
    And a configuration with game_dir at "Program Files (x86)" and active_profile "p1"
    When I save and read config.json
    Then the values return unchanged
    And the parent directories are created
    And no config.json.tmp file remains

  Scenario: Refuse corrupted JSON
    Given an existing file containing "{ not json"
    When I try to load its data
    Then I receive a read error
    And the original file remains intact
    And no default is saved over it

  Scenario: Replace existing data safely on Windows
    Given config.json contains a valid configuration
    When I save a second configuration
    Then the next read returns the complete second configuration
    And there is no pending temporary file

  Scenario: Preserve the previous version when the write fails
    Given an already saved valid file
    When an I/O failure prevents the atomic replacement
    Then the operation returns an error
    And the previous file remains intact and readable

  Scenario: An existing config with a mixed-separator game_dir is fixed on load
    Given a config.json whose game_dir is "c:/program files (x86)/steam\steamapps\common\MotoGP™26"
    When I load the configuration
    Then game_dir is "C:\program files (x86)\steam\steamapps\common\MotoGP™26"
