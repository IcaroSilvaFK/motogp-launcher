# language: en
Feature: Group mod files

  Scenario: Reject a group without a pak
    Given a group containing only "A.utoc"
    When I validate the grouping
    Then I receive an error containing "has no .pak file"

  Scenario: Accept extensions regardless of case
    Given the files "A.PAK, A.Utoc, a.UCAS"
    When I validate the grouping
    Then I receive one group named "A" with the files "A.PAK, A.Utoc, a.UCAS"

  Scenario: Ignore unrelated extensions
    Given the files "A.pak, readme.txt, A.sig"
    When I validate the grouping
    Then I receive one group named "A" with the files "A.pak"

  Scenario Outline: Strip only the final _P from the display name
    Given the base name "<base>"
    When I ask for its display name
    Then the display name is "<display>"

    Examples:
      | base  | display |
      | A_P   | A       |
      | A_P_P | A_P     |
      | A_Pro | A_Pro   |
