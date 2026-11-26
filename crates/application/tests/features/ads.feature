Feature: Choosing a sponsored creative

  Scenario: A valid creative is returned for a slot
    Given the feed has a creative for the "discover" slot
    And ads are enabled
    When the UI asks for the "discover" ad
    Then it receives the title, body, image and a "Sponsored" label
    And it does not receive the click URL

  Scenario: Ads are off
    Given the feed has a creative for the "discover" slot
    And the user turned ads off
    When the UI asks for any ad
    Then no creative is returned and nothing is fetched

  Scenario: A creative with a non-HTTPS click URL is rejected
    Given the feed has a creative whose click URL uses "http"
    When the feed is read
    Then that creative is dropped

  Scenario: An expired creative is not shown
    Given the feed has only an expired creative for the "sidebar" slot
    And ads are enabled
    When the UI asks for the "sidebar" ad
    Then no creative is returned

  Scenario: The feed is unreachable and nothing is cached
    Given the feed is unreachable and the cache is empty
    And ads are enabled
    When the UI asks for the "discover" ad
    Then no creative is returned and no error is shown

  Scenario: Two creatives share a slot
    Given the feed has two creatives for the "discover" slot
    And ads are enabled
    When the UI asks for the "discover" ad twice
    Then it receives each creative once

  Scenario: An impression is counted once per session
    Given the feed has a creative with an impression URL for the "discover" slot
    And ads are enabled
    When the UI reports the "discover" ad as shown twice
    Then exactly one impression beacon is sent

  Scenario: A house ad sends no beacon
    Given the feed has a creative without an impression URL for the "about" slot
    And ads are enabled
    When the UI reports the "about" ad as shown
    Then no impression beacon is sent

  Scenario: Clicking opens the creative's URL by id
    Given the feed has a creative for the "discover" slot
    And ads are enabled
    When the UI clicks the "discover" ad
    Then the creative's click URL is opened

  Scenario: An unknown creative id cannot be opened
    Given the feed has a creative for the "discover" slot
    When the UI clicks the "discover" ad with the id "not-offered"
    Then nothing is opened and an unknown creative error is returned
