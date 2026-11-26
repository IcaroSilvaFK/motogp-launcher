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

  Scenario: An entry with screenshots and a long description is accepted
    Given a catalog entry with 3 https screenshots and a long description
    When it is validated
    Then it keeps the 3 screenshots in order and the long description

  Scenario: Older entries stay valid
    Given a catalog entry without screenshots or long description
    When it is validated
    Then it has no screenshots and no long description

  Scenario: A non-https screenshot is refused
    Given a catalog entry with the screenshot "http://example.com/a.png"
    When it is validated
    Then it is refused naming the field "screenshots"

  Scenario: More than 8 screenshots are refused
    Given a catalog entry with 9 screenshots
    When it is validated
    Then it is refused because at most 8 screenshots are allowed
