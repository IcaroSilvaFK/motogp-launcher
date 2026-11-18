# language: en
Feature: Persist profiles safely

  Scenario: Seed a missing profiles file
    Given profiles.json does not exist yet
    When I list the profiles
    Then I receive only Vanilla with ID "vanilla" and no mods
    And profiles.json now contains Vanilla

  Scenario: Seed a valid empty collection
    Given profiles.json contains "[]"
    When I list the profiles
    Then I receive only Vanilla with ID "vanilla" and no mods
    And profiles.json now contains Vanilla

  Scenario: Roundtrip preserves the mod order
    Given a saved profile "Pinturas 2025" with mods "b", "a", "c"
    When I read the profiles again from a new store
    Then the mods of that profile are "b", "a", "c"

  Scenario: Do not reseed invalid JSON
    Given profiles.json contains "{ not json"
    When I list the profiles
    Then I receive a corrupted data error
    And the original file remains intact

  Scenario: Do not reseed a zero-byte file
    Given profiles.json contains ""
    When I list the profiles
    Then I receive a corrupted data error
    And the original file remains intact
