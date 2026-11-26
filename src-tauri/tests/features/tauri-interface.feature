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

  Scenario: The interface can query and stop the session
    Given the registered commands and controlled application ports
    Then the commands "current_session" and "stop_session" are registered
    And "current_session" answers phase "idle" with no start time on a fresh launch
    And "stop_session" without a session fails with code "no_active_session"

  Scenario: Open a catalog page through IPC
    Given a cached catalog with the entry "rebalanced-ai"
    When the frontend invokes open_catalog_page with id "rebalanced-ai"
    Then the controlled web opener receives the entry's page URL

  Scenario: Clicking an ad opens the system browser
    Given an ad feed with the creative "acme-1" in the "discover" slot
    When the frontend invokes get_ad with slot "discover"
    Then it receives the creative "acme-1" without any URL
    When the frontend invokes ad_clicked with slot "discover" and creativeId "acme-1"
    Then the controlled ad opener receives the creative's click URL
    And no game session was started

  Scenario: The UI cannot open an arbitrary URL
    Given an ad feed with the creative "acme-1" in the "discover" slot
    When the frontend invokes ad_clicked with slot "discover" and creativeId "https://evil.example"
    Then nothing is opened
    And the command fails with code "ad_unknown_creative"

  Scenario: No ad is served when the feed is empty
    Given the composed state with an empty ad feed
    When the frontend invokes get_ad with slot "discover"
    Then it receives null
