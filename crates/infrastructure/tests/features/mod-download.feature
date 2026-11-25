Feature: HTTPS mod downloader

  Scenario: Stream a file with progress
    Given a local server serves a 1 MiB file
    When it is downloaded with a 1 MiB cap
    Then the file on disk matches the served bytes
    And progress was reported up to 1 MiB

  Scenario: Resume after a dropped connection
    Given a local server drops the connection after half of the file
    When it is downloaded
    Then the second request asks for the remaining bytes with a Range header
    And the file on disk matches the served bytes

  Scenario: Stop at the declared size
    Given a local server serves more bytes than the cap
    When it is downloaded
    Then the download fails as too large

  Scenario: Give up after the retry limit
    Given a local server that always drops the connection
    When it is downloaded with 3 retries
    Then the download fails with a network error after 4 attempts

  Scenario: Refuse non-https URLs
    When a download from "http://example.com/mod.zip" is requested
    Then it fails without any request
