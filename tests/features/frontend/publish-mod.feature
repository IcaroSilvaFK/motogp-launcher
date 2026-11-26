Feature: Publishing a downloadable mod

  Scenario: A creator generates a catalog entry
    Given a local mod file and its direct HTTPS URL
    When the creator runs the catalog entry script
    Then it prints an entry with the file size and its lowercase SHA-256
    And the entry passes the catalog schema check

  Scenario: The hosted file does not match
    Given the URL serves different bytes from the local file
    When the script verifies the URL
    Then it fails and reports the size or hash mismatch

  Scenario: A non-HTTPS URL is refused
    Given the URL uses "http"
    When the creator runs the script
    Then it fails without printing an entry
