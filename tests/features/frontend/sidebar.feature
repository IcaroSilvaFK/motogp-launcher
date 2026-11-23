Feature: The sidebar remembers whether it is collapsed
  Scenario: The collapsed choice survives a restart
    Given a stored preference with the sidebar collapsed
    When the launcher opens
    Then the sidebar is collapsed
    When I expand the sidebar
    Then the stored preference says the sidebar is expanded

  Scenario: A broken preference store does not break the launcher
    Given a preference store that fails to read and to save
    When the launcher opens
    Then the sidebar is expanded
    And toggling the sidebar still collapses it
