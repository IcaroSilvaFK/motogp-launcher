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
