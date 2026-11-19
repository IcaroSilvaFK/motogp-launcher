Feature: Persist the UI language
  Scenario: The language survives a restart
    Given the user switched the language to English
    When the launcher restarts
    Then the interface is shown in English
