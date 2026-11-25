Feature: Open the support page
  The support page opens in the default browser, always with the configured URL.

  Scenario: Open the support page
    When I open the support page
    Then the opener receives the configured support URL

  Scenario: Opener failure is reported
    Given the opener fails
    When I open the support page
    Then the error is "open failed"
