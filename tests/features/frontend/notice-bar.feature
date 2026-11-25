Feature: Notice bar error messages
  The global notice bar shows errors in the user's language only.

  Scenario: A known error shows only its translated message
    Given the launcher is in English
    And a command fails with code "write_failed" and message "os error 5: Access is denied"
    When the notice bar shows the error
    Then it shows "Could not save the launcher data."
    And the raw error message is not shown

  Scenario: The translated message follows the selected language
    Given the launcher is in Portuguese
    And a command fails with code "write_failed" and message "os error 5: Access is denied"
    When the notice bar shows the error
    Then it shows the Portuguese text for "write_failed"
    And the raw error message is not shown

  Scenario: The technical detail goes to the console
    Given a command fails with code "write_failed" and message "os error 5: Access is denied"
    When the notice bar shows the error
    Then a console warning contains "write_failed" and "os error 5: Access is denied"

  Scenario: An unknown error code shows the generic fallback
    Given a command fails with code "some_future_code" and message "boom"
    When the notice bar shows the error
    Then it shows the translated generic error message
    And the raw error message is not shown
