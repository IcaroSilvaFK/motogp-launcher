Feature: Open a catalog page
  A catalog entry's web page opens in the default browser by entry id, only
  when its URL passes the page policy.

  Scenario: Open the page of a known entry
    Given the catalog holds the entry "rebalanced-ai" with page "https://overtake.gg/downloads/rebalanced-ai.69345/"
    When I open the page of "rebalanced-ai"
    Then the opener receives "https://overtake.gg/downloads/rebalanced-ai.69345/"

  Scenario: Unknown entry
    Given the catalog holds the entry "rebalanced-ai" with page "https://overtake.gg/downloads/rebalanced-ai.69345/"
    When I open the page of "missing"
    Then the error is "entry not found" and nothing is opened

  Scenario: Disallowed host
    Given the catalog holds the entry "x" with page "https://example.com/x"
    When I open the page of "x"
    Then the error is "page not allowed" and nothing is opened

  Scenario: Opener failure is reported
    Given the catalog holds the entry "rebalanced-ai" with page "https://overtake.gg/downloads/rebalanced-ai.69345/"
    And the opener fails
    When I open the page of "rebalanced-ai"
    Then the error is "open failed"
