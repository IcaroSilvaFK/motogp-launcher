Feature: Validating a sponsored creative

  Scenario: A complete creative is accepted
    Given a creative draft for the "discover" slot with an https click URL and the sponsor label "Acme Racing"
    When it is validated
    Then it is accepted with the slot "discover"

  Scenario: A creative with a non-HTTPS click URL is rejected
    Given a creative draft whose click URL is "http://example.com"
    When it is validated
    Then it is rejected because "clickUrl" is not https

  Scenario: A creative without a sponsor label is rejected
    Given a creative draft whose sponsor label is "   "
    When it is validated
    Then it is rejected because "sponsorLabel" is missing

  Scenario: A creative for an unknown slot is rejected
    Given a creative draft for the "popup" slot
    When it is validated
    Then it is rejected because the slot is unknown

  Scenario: An SVG image is refused
    Given a creative draft whose image is "data:image/svg+xml;base64,PHN2Zy8+"
    When it is validated
    Then it is rejected because the image is invalid
