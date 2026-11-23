Feature: Load order and conflicts in the UI (frontend)

  Scenario: Reorder the load order with the keyboard
    Given a profile with mods "Alpha" and "Beta" in that order
    When I move "Beta" up with the keyboard and save
    Then the profile is saved with the order "Beta", "Alpha"

  Scenario: The conflicts panel previews the draft order
    Given "Alpha" and "Beta" ship the same file
    When both are in the draft order
    Then the conflicts panel lists the file, both mods and "Beta" as the winner
    When I move "Beta" above "Alpha"
    Then the panel shows "Alpha" as the winner

  Scenario: No conflicts
    Given the draft mods share no file
    Then the conflicts panel says there are no conflicts

  Scenario: Library rows show a conflict badge for the active profile
    Given the active profile contains conflicting mods "Alpha" then "Beta"
    When I open the library
    Then the "Alpha" row shows an "overridden" conflict badge
    And the "Beta" row shows a "wins" conflict badge
    And mods without conflicts show no conflict badge
