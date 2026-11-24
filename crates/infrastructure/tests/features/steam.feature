# language: en
Feature: Find the correct MotoGP installation on Steam
  Uses a temporary Steam tree; the real registry is a separate manual smoke.

  Scenario: Parse Steam libraries with escaped backslashes
    Given a libraryfolders.vdf with "C:\\Program Files (x86)\\Steam" and "D:\\SteamLibrary" escaped in VDF format
    When I read the libraries
    Then I receive both Windows paths with unescaped backslashes
    And the spaces are preserved

  Scenario: Read the App ID and the install directory
    Given an ACF manifest with appid "123" and installdir "MotoGP25"
    When I parse the manifest
    Then I receive the ID 123 and the name "MotoGP25"

  Scenario: Inspect a valid Unreal game
    Given "common/MotoGP25/MotoGP25/Content/Paks" in a temporary Steam tree
    And an appmanifest in steamapps with installdir "motogp25" and appid "123"
    When I inspect the root "common/MotoGP25"
    Then mods_dir ends with "MotoGP25/Content/Paks/~mods"
    And the App ID is 123
    And the ~mods folder is not created

  Scenario: Reject a root without project Paks
    Given a root without Content/Paks in its project subfolders
    When I inspect it
    Then I receive an error containing "Content/Paks not found"

  Scenario: Detect only MotoGP
    Given libraries with a valid MotoGP25 and a valid OtherGame
    When I detect the games
    Then only MotoGP25 appears

  Scenario: Ignore the Engine folder
    Given a root with Engine/Content/Paks and Project/Content/Paks
    When I inspect it
    Then the Paks folder used is Project/Content/Paks

  Scenario: Allow a manual folder without a Steam manifest
    Given a valid Unreal root without a matching manifest
    When I inspect it
    Then I receive a GameInfo with no App ID
    And no mods folder is created

  Scenario: Steam not found in the registry
    Given SteamPath is not available
    When I detect the games
    Then I receive an empty list
    And manual inspection is still available

  Scenario: Normalize the Steam path read from the registry
    Given the registry SteamPath is "c:/program files (x86)/steam"
    When I normalize it
    Then I receive "C:\program files (x86)\steam"

  Scenario: Prefer the library spelling from libraryfolders.vdf
    Given the registry SteamPath is a lower-cased forward-slash spelling of the Steam folder
    And a libraryfolders.vdf that lists the real spelling plus "D:\\SteamLibrary"
    When I read the libraries
    Then the first library has the VDF spelling and the second is "D:\SteamLibrary"
    And no path contains a forward slash

  Scenario: Accented and trademark characters survive normalization
    # "Usuário Padrão" is a real-world accented Windows folder name; kept on purpose.
    Given the path "c:/Users/Usuário Padrão/Steam\steamapps\common\MotoGP™26"
    When I normalize it
    Then I receive "C:\Users\Usuário Padrão\Steam\steamapps\common\MotoGP™26"
