# language: en
Feature: Load order and conflict detection (domain)

  Scenario: Later mod in the load order wins a conflict
    Given mod "a" ships "Ducati_P.pak" and mod "b" ships "ducati_p.pak"
    When conflicts are detected for the order "a", "b"
    Then there is one conflict on "ducati_p.pak" involving "a" then "b"
    And the winner is "b"

  Scenario: Reordering changes the winner
    Given mod "a" ships "Ducati_P.pak" and mod "b" ships "ducati_p.pak"
    When conflicts are detected for the order "b", "a"
    Then the winner is "a"

  Scenario: Mods without shared files do not conflict
    Given mod "a" ships "A_P.pak" and mod "b" ships "B_P.pak"
    When conflicts are detected for the order "a", "b"
    Then no conflict is detected

  Scenario: Missing and duplicate mods are ignored
    Given mod "a" ships "A_P.pak" and mod "b" ships "A_P.pak"
    When conflicts are detected for the order "a", "zzz", "a"
    Then no conflict is detected

  Scenario: The load order has no duplicates
    When the load order is set to "c", "a", "c", "b"
    Then the profile order is "c", "a", "b"
