# language: en
Feature: Load order use cases (application)

  Scenario: Saving the load order persists it normalized
    Given an existing profile
    When I set its mods to "b", "a", "b"
    Then reading it returns exactly "b", "a"

  Scenario: Listing conflicts for a candidate order
    Given a library where "a" and "b" ship the same file
    When I list the conflicts for "a", "b"
    Then I get that file with mods "a", "b" and winner "b"

  Scenario: Deploy follows the load order
    Given a profile ordered "b", "a" whose mods ship the same file
    When the profile is deployed
    Then the file of "b" gets the prefix "000" and the file of "a" gets the prefix "001"
    And the winner "a" loads last
