# language: en
Feature: Find the correct MotoGP installation
  Use cases run against fake ports (no Steam, no disk).

  Scenario: Inspect a valid Unreal game
    Given the game folder "common/MotoGP25" has the project "MotoGP25" with Content/Paks
    And a Steam manifest with installdir "motogp25" and appid "123"
    When I inspect the root "common/MotoGP25"
    Then mods_dir ends with "MotoGP25/Content/Paks/~mods"
    And the App ID is 123

  Scenario: Reject a root without project Paks
    Given a root "common/Empty" without Content/Paks in its project folders
    When I inspect the root "common/Empty"
    Then I receive an error containing "Content/Paks not found"

  Scenario: Detect only MotoGP
    Given libraries with a valid "MotoGP25" and a valid "OtherGame"
    When I detect the games
    Then only "MotoGP25" appears

  Scenario: Ignore the Engine folder
    Given a root "common/MotoGP25" with Engine/Content/Paks and Bike/Content/Paks
    When I inspect the root "common/MotoGP25"
    Then the Paks folder used belongs to "Bike"

  Scenario: Allow a manual folder without a Steam manifest
    Given the game folder "manual/MotoGP25" has the project "MotoGP25" with Content/Paks
    When I inspect the root "manual/MotoGP25"
    Then I receive a GameInfo with no App ID

  Scenario: Steam not found in the registry
    Given SteamPath is not available
    When I detect the games
    Then I receive an empty list
    And manual inspection is still available
