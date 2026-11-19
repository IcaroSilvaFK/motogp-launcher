# language: en
Feature: Organize imported mods in the library

  Scenario: Import a loose pak
    Given a file "A.pak"
    When I import it
    Then I receive one entry with the file "A.pak"
    And its ID has 12 lowercase hexadecimal characters derived from the hash of the pak
    And the index and library/<id>/A.pak contain the imported mod

  Scenario: Pick up only the siblings of the chosen mod
    Given "A_P.pak", "A_P.utoc", "A_P.ucas" and "B.pak" in the same folder
    When I import "A_P.pak"
    Then the mod is named "A" and has the three files of A_P
    And B is not imported

  Scenario: Find mods in the subfolders of a ZIP
    Given a ZIP containing "x/y/A.pak" and "B.pak"
    When I import it
    Then I receive two entries with flattened file names

  Scenario: Reject a ZIP without a pak
    Given a ZIP containing only "readme.txt"
    When I import it
    Then I receive an error containing "no .pak"

  Scenario: Reimport without duplicating
    Given a pak that was already imported
    When I import the same content again
    Then I receive the existing entry
    And the library still has one entry for that ID

  Scenario: Remove a mod
    Given an imported mod
    When I remove its ID
    Then its folder and index entry no longer exist

  Scenario: Reject an unsupported format
    Given a file "mod.rar"
    When I try to import it
    Then I receive an error containing "unsupported format"

  Scenario: A failed import publishes nothing
    Given a file "A.pak" that cannot be copied
    When I try to import it
    Then I receive an error
    And the index stays empty
    And no temporary files remain
