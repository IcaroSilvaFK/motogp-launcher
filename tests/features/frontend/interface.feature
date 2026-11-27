Feature: Use the launcher through the interface
  Scenario: Configure an Unreal folder without Steam
    Given the fake folder "C:/temp/MotoGPFake/MotoGPFake/Content/Paks"
    When I choose "C:/temp/MotoGPFake" in Settings
    Then I see mods_dir ending in "Content/Paks/~mods"
    And I see the App ID as not found

  Scenario: Use a detected game
    Given detection returns a MotoGP with an App ID
    When I click "Use this"
    Then the game configuration is persisted
    And the current game is updated on screen

  Scenario: Import through the dialog without duplicating
    Given a test pak not yet in the library
    When I select it in Import
    Then the library shows its name, size and file count
    When I import the same pak again
    Then there is still a single entry for the mod

  Scenario: Import by drag and drop
    Given the Library screen open
    When I receive a drop event with supported files
    Then the paths are sent to the import use case
    And the list is reloaded

  Scenario: Cancel the selection and remove the listener
    Given an open dialog and active listeners on the screen
    When I cancel the dialog and then unmount the screen
    Then no import is requested
    And the listeners are released without duplicated callbacks

  Scenario: Create a profile and order mods
    Given mods A and B in the library
    When I create the profile "Teste", check the mods and move B before A
    Then I save mod_ids in the order B, A
    And the order remains after reloading the profile

  Scenario: Rename, delete and protect profiles
    Given a regular profile and Vanilla
    When I rename the regular profile and then delete it after confirming
    Then the list reflects both changes
    And Vanilla offers no delete action

  Scenario: Deleting a profile asks first
    Given a regular profile and Vanilla
    When I click Delete on the regular profile
    Then a confirmation appears and the profile is not deleted yet
    When I cancel with Esc
    Then the profile is still listed

  Scenario: Remove a mod from the library and from profiles
    Given a listed mod referenced in a profile
    When I remove it from the Library after confirming
    Then it no longer appears in the library nor in the profile selection

  Scenario: Removing a mod asks first
    Given a listed mod referenced in a profile
    When I click Remove on the mod
    Then a confirmation appears and the mod is not removed yet
    When I cancel with the Cancel button
    Then the mod stays installed

  Scenario: A failed removal is reported
    Given a listed mod whose removal fails
    When I remove it from the Library after confirming
    Then I see the error in the accessible notice at the top

  Scenario: Warn of an error when playing without an App ID
    Given the profile "Teste" active and a game configured without an App ID
    When I click Play
    Then I see a notice containing "game App ID not found"
    And no file is created in ~mods

  Scenario: Control Play availability
    Given the app without a configured game
    Then Play is disabled
    When I configure a game and request to start a session
    Then Play is replaced by Stop during start and execution

  Scenario: Show session events
    Given a requested session
    When I receive session with state "started"
    Then I see "Running…"
    When I receive session with state "ended" and cleaned 2
    Then I see "Session ended — 2 files removed"
    And Play becomes available again

  Scenario: Show that the game did not open
    Given a requested session
    When I receive session with state "never-started" after the cleanup finishes
    Then I see "The game did not open within 2 min; mods removed"

  Scenario: Show operation errors
    Given an operation whose port returns an error
    When the operation finishes
    Then I see the error in the accessible notice at the top
    And I do not see a false success confirmation
