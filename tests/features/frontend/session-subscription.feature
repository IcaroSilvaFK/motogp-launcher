Feature: Session event subscription failure
  Scenario: The session event subscription fails
    Given the session event subscription rejects
    When the launcher starts
    Then an error notice is announced
    And Play stays disabled
