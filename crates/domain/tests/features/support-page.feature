Feature: Support page URL policy
  The support page opens only when it is a plain https URL on its own host,
  and that host is not part of the catalog allow-list.

  Scenario: The configured support URL is allowed
    Given the configured support URL
    Then the support URL is allowed

  Scenario: A subdomain of the support host is allowed
    Given the support URL "https://www.apoia.se/x"
    Then the support URL is allowed

  Scenario Outline: Lookalike hosts and plain http are rejected
    Given the support URL "<url>"
    Then the support URL is rejected

    Examples:
      | url                        |
      | https://evilapoia.se/      |
      | https://apoia.se.evil.com/ |
      | http://apoia.se/           |

  Scenario: The catalog policy does not allow the support host
    Given the support URL "https://apoia.se/x"
    Then the catalog policy rejects it as host not allowed
