# language: en
Feature: Import mod files into the library

  Scenario: Import a loose pak
    Given a file "A.pak" with the content "abc"
    When I import it
    Then I receive an entry with the file "A.pak"
    And its ID is "ba7816bf8f01", the first 12 hex characters of the SHA-256 of the pak
    And library.json and library/<id>/A.pak contain the imported mod

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

  Scenario: Ignore directory traversal in a ZIP
    Given a ZIP containing "../evil.pak" and "ok.pak"
    When I import it
    Then only "ok.pak" enters the library
    And no file is written outside the controlled directory

  Scenario: Reject a collision when flattening subfolders
    Given a ZIP containing "x/A.pak" and "y/A.pak" with different contents
    And a mod already in the library
    When I import it
    Then I receive a collision error
    And no existing library file is overwritten

  Scenario: Reject an unsupported format
    Given a file "mod.rar" with the content "rar"
    When I try to import it
    Then I receive an error containing "unsupported format"
