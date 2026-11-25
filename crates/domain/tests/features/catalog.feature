Feature: Catalog entries (domain)
  Scenario: A complete entry is valid
    Given a draft with every required field and an https page URL
    When it is validated
    Then a catalog entry is produced

  Scenario: Reject a non-https URL
    Given a draft whose page URL is "http://example.com/mod"
    When it is validated
    Then it is rejected with an "insecure URL" reason

  Scenario: Reject a download without a valid checksum
    Given a draft whose download has a size but a 10-character sha256
    When it is validated
    Then it is rejected

  Scenario: Reject a blank title
    Given a draft whose title is "   "
    When it is validated
    Then it is rejected
