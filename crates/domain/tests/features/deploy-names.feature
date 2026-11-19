# language: en
Feature: Name the deployed files of a session

  Scenario: Deploy in the profile order
    Given the mods "B" and "A" in that order, with the IDs "bbbbbbbbbbbb" and "aaaaaaaaaaaa"
    When I compute the destination of their files
    Then the destinations are "000_bbbbbbbbbbbb_B.pak" and "001_aaaaaaaaaaaa_A.pak"

  Scenario: Preserve the patch suffix
    Given an original file "Ducati_P.pak"
    When I compute its destination
    Then the name ends in "_Ducati_P.pak"

  Scenario: Files of the same mod share the position index
    Given the mod "A" at position 2 with the files "A_P.pak, A_P.utoc, A_P.ucas"
    When I compute the destination of their files
    Then all destinations start with "002_"

  Scenario: Reject names that could leave the authorized directory
    Given a file name "..\evil.pak"
    When I check whether the name is safe
    Then the name is rejected

  Scenario: Recognize the game mods folder
    Given a folder path "C:\Game\Content\Paks\~MODS"
    When I check whether the folder is a mods folder
    Then the folder is accepted

  Scenario: Reject a folder that is not named mods
    Given a folder path "C:\Game\Content\Paks\~mods.bak"
    When I check whether the folder is a mods folder
    Then the folder is rejected
