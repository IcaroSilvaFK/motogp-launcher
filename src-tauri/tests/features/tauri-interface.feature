Feature: Expose the launcher use cases through Tauri commands

  Scenario: Preserve the command contract
    Given the registered commands and controlled application ports
    When I send camelCase arguments through IPC
    Then each command forwards the expected operation
    And responses use the documented snake_case fields
    And errors are presented without business rules in the command

  Scenario: Run recovery before accepting Play
    Given a previous deployment left in ~mods by a crash
    When the application setup runs
    Then the recovery cleans the launcher files before the first Play is accepted
    And the cleaned amount is recorded

  Scenario: Keep the startup recovery result readable
    Given a startup recovery that failed before the frontend listened
    When the frontend reads recovery_status
    Then it receives the failed state with the error message

  Scenario: Keep the interface responsive and consistent
    Given concurrent imports through IPC
    When the commands run on the blocking pool
    Then every import ends up in the library index

  Scenario: Allow a single launcher instance
    Given a launcher instance holding the OS lock
    When a second instance tries to acquire it
    Then it is refused without touching the deployment state

  Scenario: set_profile_mods is refused while a session is running
    Given the game is running after Play
    When the interface calls "set_profile_mods" for a profile
    Then the command fails with code "profile_locked_by_session"
    And the profile keeps its previous mods

  Scenario: remove_mod is refused while a session is running
    Given the game is running after Play
    When the interface calls "remove_mod" for a library mod
    Then the command fails with code "library_locked_by_session"
    And the mod is still in the library
