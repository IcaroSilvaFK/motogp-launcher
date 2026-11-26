Feature: Fetching and caching ads

  Scenario: Offline with a fresh cache
    Given a creative was cached 1 day ago and the expiry is 7 days
    And the network is unavailable
    When the UI asks for the ad
    Then the cached creative is returned

  Scenario: Offline with an expired cache
    Given the cached creative expired
    And the network is unavailable
    When the UI asks for the ad
    Then no creative is returned and no error is shown

  Scenario: An oversized or non-image creative is refused
    Given the feed points to a 5 MiB image
    When the creative is fetched
    Then it is rejected and the previous cache is kept

  Scenario Outline: A creative whose image is not a raster picture is refused
    Given the feed points to an image served as "<content type>"
    When the creative is fetched
    Then it is rejected and the previous cache is kept

    Examples:
      | content type  |
      | image/svg+xml |
      | text/html     |
