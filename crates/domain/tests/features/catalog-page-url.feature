Feature: Catalog page URL policy
  Only https pages on allowed hosts may be opened from the catalog.

  Scenario: Accept an https page on an allowed host
    Given the page URL "https://overtake.gg/downloads/some-mod.1/"
    Then the URL is allowed

  Scenario: Reject a non-https page
    Given the page URL "http://overtake.gg/downloads/x/"
    Then the URL is rejected

  Scenario: Reject a host outside the allow-list
    Given the page URL "https://evilovertake.gg/x"
    Then the URL is rejected

  Scenario: Reject credentials in the URL
    Given the page URL "https://user@overtake.gg/x"
    Then the URL is rejected
